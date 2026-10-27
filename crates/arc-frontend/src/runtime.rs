use libarc::ArcDaemonProxy;
use std::sync::{Mutex, OnceLock};
use tokio::runtime::Runtime;

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

static PROXY: Mutex<Option<ArcDaemonProxy<'static>>> = Mutex::new(None);

pub async fn proxy() -> Option<ArcDaemonProxy<'static>> {
    if let Some(p) = PROXY.lock().unwrap().clone() {
        return Some(p);
    }
    match libarc::connect().await {
        Ok(p) => {
            *PROXY.lock().unwrap() = Some(p.clone());
            Some(p)
        }
        Err(e) => {
            tracing::warn!("failed to connect to daemon: {e}");
            None
        }
    }
}

pub fn invalidate_proxy() {
    *PROXY.lock().unwrap() = None;
}

// a hung daemon must never leave a page spinning forever
const CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub async fn call<T>(what: &str, request: impl std::future::Future<Output = anyhow::Result<T>>) -> anyhow::Result<T> {
    match tokio::time::timeout(CALL_TIMEOUT, request).await {
        Ok(result) => result,
        Err(_) => {
            // the next request opens a fresh connection in case this one belongs to a daemon that went away
            invalidate_proxy();
            Err(anyhow::anyhow!("{what} timed out"))
        }
    }
}
