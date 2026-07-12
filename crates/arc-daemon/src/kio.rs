use std::sync::mpsc;
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
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        if hidden {
            return Self { tx };
        }
        let title = title.to_string();
        let package_id = package_id.to_string();
        std::thread::spawn(move || run(title, package_id, cancel_token, rx));
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
    rx: mpsc::Receiver<Msg>,
) {
    // no jobview server on the session bus, drain quietly so senders never error
    let Some(job) = start_job(&title, cancel_token.is_some()) else {
        while rx.recv().is_ok() {}
        return;
    };
    let _ = job.set_description(1, &tr!("Package"), &package_id);

    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Msg::Progress(pct)) => {
                let _ = job.set_percent(pct as u32);
            }
            Ok(Msg::Finish { success: true, .. }) => {
                let _ = job.finish();
                return;
            }
            Ok(Msg::Finish { success: false, message }) => {
                let _ = job.fail(1, &message);
                return;
            }
            // wake up periodically to poll the cancel flag
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if let Some(token) = &cancel_token {
            if job.cancel_requested() {
                token.cancel();
            }
        }
    }
}
