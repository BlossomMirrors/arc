use super::PackageProvider;
use async_trait::async_trait;
use futures_util::StreamExt;
use libarc::{ArcError, Package as ArcPackage, Provider};
use std::sync::Arc;
use tracing::warn;
use zbus::{proxy, zvariant::OwnedObjectPath, Connection};

const FILTER_NONE: u64 = 1 << 1;
const FILTER_INSTALLED: u64 = 1 << 2;
const PK_INFO_INSTALLED: u32 = 1;
const PK_EXIT_SUCCESS: u32 = 1;
const TX_FLAG_NONE: u64 = 0;

#[proxy(
    interface = "org.freedesktop.PackageKit",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
trait PackageKit {
    async fn create_transaction(&self) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.PackageKit.Transaction",
    default_service = "org.freedesktop.PackageKit"
)]
trait PackageKitTransaction {
    async fn set_hints(&self, hints: Vec<String>) -> zbus::Result<()>;
    async fn search_names(&self, filter: u64, values: Vec<String>) -> zbus::Result<()>;
    async fn get_packages(&self, filter: u64) -> zbus::Result<()>;
    async fn get_updates(&self, filter: u64) -> zbus::Result<()>;
    async fn install_packages(
        &self,
        transaction_flags: u64,
        package_ids: Vec<String>,
    ) -> zbus::Result<()>;
    async fn remove_packages(
        &self,
        transaction_flags: u64,
        package_ids: Vec<String>,
        allow_deps: bool,
        autoremove: bool,
    ) -> zbus::Result<()>;
    async fn update_packages(
        &self,
        transaction_flags: u64,
        package_ids: Vec<String>,
    ) -> zbus::Result<()>;
    async fn resolve(&self, filter: u64, packages: Vec<String>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn package(&self, info: u32, package_id: String, summary: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn finished(&self, exit_enum: u32, runtime: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn error_code(&self, code: u32, details: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn item_progress(&self, package_id: String, status: u32, percentage: u32) -> zbus::Result<()>;
}

pub struct PackageKitProvider {
    connection: Arc<Connection>,
}

impl PackageKitProvider {
    pub async fn new() -> Result<Self, ArcError> {
        let connection = Connection::system().await.map_err(|e| {
            ArcError::ProviderError(format!("Failed to connect to system D-Bus: {}", e))
        })?;
        Ok(Self {
            connection: Arc::new(connection),
        })
    }

    async fn transaction(&self) -> Result<PackageKitTransactionProxy<'_>, ArcError> {
        let pk = PackageKitProxy::new(&self.connection)
            .await
            .map_err(|e| ArcError::ProviderError(format!("PackageKit proxy: {}", e)))?;

        let path = pk
            .create_transaction()
            .await
            .map_err(|e| ArcError::ProviderError(format!("CreateTransaction: {}", e)))?;

        PackageKitTransactionProxy::builder(&self.connection)
            .path(path)
            .map_err(|e| ArcError::ProviderError(format!("Transaction path: {}", e)))?
            .build()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Transaction proxy: {}", e)))
    }

    async fn interactive_transaction(&self) -> Result<PackageKitTransactionProxy<'_>, ArcError> {
        let tx = self.transaction().await?;
        if let Err(e) = tx.set_hints(vec!["interactive=true".to_string()]).await {
            warn!(
                "Failed to set interactive hint on PackageKit transaction: {}",
                e
            );
        }
        Ok(tx)
    }
}

fn parse_pk_package(
    pkg_id: &str,
    summary: &str,
    info: u32,
    installed_override: Option<bool>,
) -> ArcPackage {
    let parts: Vec<&str> = pkg_id.splitn(4, ';').collect();
    let name = parts.first().copied().unwrap_or("").to_string();
    let version = parts.get(1).copied().unwrap_or("").to_string();
    let data = parts.get(3).copied().unwrap_or("");

    let provider =
        if data.contains("flatpak") || data.contains("flathub") || name.matches('.').count() >= 2 {
            Provider::Flatpak
        } else {
            Provider::Native
        };

    let installed = installed_override.unwrap_or(info == PK_INFO_INSTALLED);

    ArcPackage {
        id: pkg_id.to_string(),
        name,
        version,
        description: summary.to_string(),
        provider,
        installed,
    }
}

macro_rules! collect_packages {
    ($tx:expr, $call:expr, $installed_override:expr) => {{
        let mut pkg_stream = $tx
            .receive_package()
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        let mut fin_stream = $tx
            .receive_finished()
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        let mut err_stream = $tx
            .receive_error_code()
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        $call
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        let mut packages: Vec<ArcPackage> = Vec::new();
        loop {
            tokio::select! {
                biased;
                pkg = pkg_stream.next() => {
                    match pkg {
                        Some(sig) => {
                            if let Ok(args) = sig.args() {
                                packages.push(parse_pk_package(
                                    args.package_id(),
                                    args.summary(),
                                    *args.info(),
                                    $installed_override,
                                ));
                            }
                        }
                        None => break,
                    }
                }
                err = err_stream.next() => {
                    if let Some(sig) = err {
                        if let Ok(args) = sig.args() {
                            return Err(ArcError::ProviderError(args.details().to_string()));
                        }
                    }
                }
                _fin = fin_stream.next() => break,
            }
        }
        packages
    }};
}

macro_rules! run_transaction {
    ($tx:expr, $call:expr) => {{
        let mut fin_stream = $tx
            .receive_finished()
            .await
            .map_err(|e| ArcError::TransactionFailed(e.to_string()))?;
        let mut err_stream = $tx
            .receive_error_code()
            .await
            .map_err(|e| ArcError::TransactionFailed(e.to_string()))?;

        $call
            .await
            .map_err(|e| ArcError::TransactionFailed(e.to_string()))?;

        let mut last_error: Option<String> = None;
        loop {
            tokio::select! {
                biased;
                err = err_stream.next() => {
                    if let Some(sig) = err {
                        if let Ok(args) = sig.args() {
                            last_error = Some(args.details().to_string());
                        }
                    }
                }
                fin = fin_stream.next() => {
                    if let Some(sig) = fin {
                        if let Ok(args) = sig.args() {
                            if *args.exit_enum() != PK_EXIT_SUCCESS {
                                let msg = match last_error {
                                    Some(details) => details,
                                    None => format!(
                                        "PackageKit exit code {}",
                                        args.exit_enum()
                                    ),
                                };
                                return Err(ArcError::TransactionFailed(msg));
                            }
                        }
                    }
                    break;
                }
            }
        }
    }};
}

impl PackageKitProvider {
    pub async fn fetch_all(&self) -> Result<Vec<ArcPackage>, ArcError> {
        let tx = self.transaction().await?;
        let packages = collect_packages!(tx, tx.get_packages(FILTER_NONE), None);
        Ok(packages)
    }
}

#[async_trait]
impl PackageProvider for PackageKitProvider {
    async fn search(&self, query: &str) -> Result<Vec<ArcPackage>, ArcError> {
        let tx = self.transaction().await?;
        let values = vec![query.to_string()];
        let packages = collect_packages!(tx, tx.search_names(FILTER_NONE, values), None);
        Ok(packages)
    }

    async fn list_installed(&self) -> Result<Vec<ArcPackage>, ArcError> {
        let tx = self.transaction().await?;
        let packages = collect_packages!(tx, tx.get_packages(FILTER_INSTALLED), Some(true));
        Ok(packages)
    }

    async fn list_updates(&self) -> Result<Vec<ArcPackage>, ArcError> {
        let tx = self.transaction().await?;
        let packages = collect_packages!(tx, tx.get_updates(FILTER_NONE), Some(true));
        Ok(packages)
    }

    async fn install(&self, package_id: &str) -> Result<(), ArcError> {
        let tx = self.interactive_transaction().await?;
        let ids = vec![package_id.to_string()];
        run_transaction!(tx, tx.install_packages(TX_FLAG_NONE, ids));
        Ok(())
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        let name = package_id
            .split(';')
            .next()
            .unwrap_or(package_id)
            .to_string();

        let resolve_tx = self.transaction().await?;
        let installed = collect_packages!(
            resolve_tx,
            resolve_tx.resolve(FILTER_INSTALLED, vec![name.clone()]),
            Some(true)
        );

        let resolved_id = installed
            .first()
            .map(|p| p.id.clone())
            .ok_or_else(|| ArcError::PackageNotFound(format!("{} is not installed", name)))?;

        let tx = self.interactive_transaction().await?;
        let ids = vec![resolved_id];
        run_transaction!(tx, tx.remove_packages(TX_FLAG_NONE, ids, true, false));
        Ok(())
    }

    async fn update(&self, package_id: &str) -> Result<(), ArcError> {
        let tx = self.interactive_transaction().await?;
        let ids = vec![package_id.to_string()];
        run_transaction!(tx, tx.update_packages(TX_FLAG_NONE, ids));
        Ok(())
    }
}
