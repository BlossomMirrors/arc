use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tr::tr;

// krateio speaks the blocking zbus API so each job gets its own thread,
// fed through a channel from the async transaction tasks
enum Msg {
    Progress(u8),
    Finish { success: bool, message: String },
}

#[derive(Clone)]
pub struct KioJob {
    tx: mpsc::Sender<Msg>,
}

impl KioJob {
    // hidden skips the job view entirely so no notification appears while
    // the sender side keeps working as usual
    pub fn start(
        title: &str,
        package_id: &str,
        cancel_token: Option<CancellationToken>,
        hidden: bool,
        suppressed: Arc<AtomicBool>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        if hidden {
            return Self { tx };
        }
        let title = title.to_string();
        let package_id = package_id.to_string();
        std::thread::spawn(move || run(title, package_id, cancel_token, suppressed, rx));
        Self { tx }
    }

    pub fn progress(&self, pct: u8) {
        let _ = self.tx.send(Msg::Progress(pct));
    }

    pub fn finish(&self, success: bool, message: &str) {
        let _ = self.tx.send(Msg::Finish {
            success,
            message: message.to_string(),
        });
    }
}

fn start_job(title: &str, cancellable: bool) -> Option<krateio::Job> {
    let tracker = krateio::Tracker::new("org.blossomos.Arc")
        .ok()?
        .with_app_name("Arc")
        .with_app_icon("org.blossomos.Arc");
    tracker
        .job()
        .title(title)
        .cancellable(cancellable)
        .start()
        .ok()
}

fn run(
    title: String,
    package_id: String,
    cancel_token: Option<CancellationToken>,
    suppressed: Arc<AtomicBool>,
    rx: mpsc::Receiver<Msg>,
) {
    let mut job: Option<krateio::Job> = None;
    // no jobview server on the session bus, drain quietly so senders never error
    let mut jobview_missing = false;
    let mut percent = 0u8;

    loop {
        let msg = rx.recv_timeout(Duration::from_millis(500));

        if job.is_none() && !jobview_missing && !suppressed.load(Ordering::Relaxed) {
            match start_job(&title, cancel_token.is_some()) {
                Some(j) => {
                    let _ = j.set_description(1, &tr!("Package"), &package_id);
                    let _ = j.set_percent(percent as u32);
                    job = Some(j);
                }
                None => jobview_missing = true,
            }
        }

        match msg {
            Ok(Msg::Progress(pct)) => {
                percent = pct;
                if let Some(job) = &job {
                    let _ = job.set_percent(pct as u32);
                }
            }
            Ok(Msg::Finish { success: true, .. }) => {
                if let Some(job) = job.take() {
                    let _ = job.finish();
                }
                return;
            }
            Ok(Msg::Finish { success: false, message }) => {
                if let Some(job) = job.take() {
                    let _ = job.fail(1, &message);
                }
                return;
            }
            // wake up periodically to poll the cancel flag
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }

        if let (Some(job), Some(token)) = (&job, &cancel_token) {
            if job.cancel_requested() {
                token.cancel();
            }
        }
    }
}
