use crate::cache::disk;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::OnceLock;
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(750);

type Encode = Box<dyn FnOnce() -> Vec<u8> + Send>;

enum Msg {
    Write(PathBuf, Encode),
    Flush(SyncSender<()>),
}

static SENDER: OnceLock<SyncSender<Msg>> = OnceLock::new();

fn sender() -> &'static SyncSender<Msg> {
    SENDER.get_or_init(|| {
        let (tx, rx) = sync_channel(4096);
        std::thread::Builder::new()
            .name("arc-cache-writer".into())
            .spawn(move || run(rx))
            .expect("failed to spawn cache writer thread");
        tx
    })
}

fn run(rx: Receiver<Msg>) {
    let mut pending: HashMap<PathBuf, Encode> = HashMap::new();
    loop {
        let msg = if pending.is_empty() {
            match rx.recv() {
                Ok(msg) => msg,
                Err(_) => return,
            }
        } else {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(msg) => msg,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    drain(&mut pending);
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    drain(&mut pending);
                    return;
                }
            }
        };

        match msg {
            Msg::Write(path, encode) => {
                pending.insert(path, encode);
            }
            Msg::Flush(ack) => {
                drain(&mut pending);
                let _ = ack.send(());
            }
        }
    }
}

fn drain(pending: &mut HashMap<PathBuf, Encode>) {
    for (path, encode) in pending.drain() {
        let bytes = encode();
        let _ = disk::write_atomic(&path, &bytes);
    }
}

pub fn enqueue(path: PathBuf, bytes: Vec<u8>) {
    let _ = sender().send(Msg::Write(path, Box::new(move || bytes)));
}

// like enqueue but defers producing the bytes to the Writer thread instead of paying that cost on the callers critical path
pub fn enqueue_with(path: PathBuf, encode: impl FnOnce() -> Vec<u8> + Send + 'static) {
    let _ = sender().send(Msg::Write(path, Box::new(encode)));
}

pub fn flush_blocking() {
    let (ack_tx, ack_rx) = sync_channel(1);
    if sender().send(Msg::Flush(ack_tx)).is_ok() {
        let _ = ack_rx.recv();
    }
}
