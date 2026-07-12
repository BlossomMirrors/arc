use libarc::ArcDaemonProxy;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tokio::sync::OnceCell;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn init() {
    RUNTIME
        .set(Runtime::new().expect("failed to start tokio runtime"))
        .expect("runtime already initialized");
}

pub fn spawn<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    RUNTIME
        .get()
        .expect("runtime::init() was not called")
        .spawn(future);
}

static PROXY: OnceCell<ArcDaemonProxy<'static>> = OnceCell::const_new();

pub async fn proxy() -> Option<ArcDaemonProxy<'static>> {
    match PROXY.get_or_try_init(libarc::connect).await {
        Ok(p) => Some(p.clone()),
        Err(e) => {
            tracing::warn!("failed to connect to daemon: {e}");
            None
        }
    }
}
