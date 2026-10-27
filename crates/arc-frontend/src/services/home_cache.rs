use super::home::{HomeSection, Story};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    sections: Vec<HomeSection>,
    stories: Vec<Story>,
}

pub fn load() -> Option<Vec<HomeSection>> {
    let snapshot = crate::cache::home().get(&super::forge::user_lang())?;
    super::stories::seed(&snapshot.stories);
    Some(snapshot.sections)
}

pub struct Preloaded {
    pub raw: Vec<HomeSection>,
    pub display: Vec<HomeSection>,
}

static PRELOADED: std::sync::Mutex<Option<Preloaded>> = std::sync::Mutex::new(None);
static PRELOAD_DONE: tokio::sync::Notify = tokio::sync::Notify::const_new();
static PRELOAD_FINISHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const RESOLVE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

pub fn preload() {
    crate::runtime::spawn(async {
        let raw = tokio::task::spawn_blocking(load).await.ok().flatten();
        if let Some(raw) = raw {
            let mut display = raw.clone();
            let _ = tokio::time::timeout(RESOLVE_TIMEOUT, async {
                let proxy = crate::runtime::proxy().await;
                super::images::resolve_sections(proxy.as_ref(), &mut display).await;
            })
            .await;
            *PRELOADED.lock().unwrap_or_else(|p| p.into_inner()) = Some(Preloaded { raw, display });
        }
        PRELOAD_FINISHED.store(true, std::sync::atomic::Ordering::Release);
        PRELOAD_DONE.notify_waiters();
    });
}

pub fn take_preloaded() -> Option<Preloaded> {
    PRELOADED.lock().unwrap_or_else(|p| p.into_inner()).take()
}

pub async fn wait_preloaded() -> Option<Preloaded> {
    let done = PRELOAD_DONE.notified();
    if !PRELOAD_FINISHED.load(std::sync::atomic::Ordering::Acquire) {
        done.await;
    }
    take_preloaded()
}

pub fn save(sections: &[HomeSection], stories: &[Story]) {
    let snapshot = Snapshot {
        sections: sections.to_vec(),
        stories: stories.to_vec(),
    };
    crate::cache::home().insert(super::forge::user_lang(), snapshot);
}
