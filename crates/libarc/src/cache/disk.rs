use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), seq));

    let result = (|| {
        let file = fs::File::create(&tmp)?;
        {
            let mut file = &file;
            io::Write::write_all(&mut file, bytes)?;
        }
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        if let Some(dir) = path.parent() {
            if let Ok(dir_file) = fs::File::open(dir) {
                let _ = dir_file.sync_all();
            }
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn write_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), seq));
    let result = fs::write(&tmp, bytes).and_then(|_| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn read(path: &Path) -> Option<Vec<u8>> {
    fs::read(path).ok()
}

pub fn age(path: &Path) -> Option<Duration> {
    fs::metadata(path).ok()?.modified().ok()?.elapsed().ok()
}

pub fn touch(path: &Path) {
    if let Ok(file) = fs::File::options().append(true).open(path) {
        let _ = file.set_modified(std::time::SystemTime::now());
    }
}

pub fn remove(path: &Path) {
    let _ = fs::remove_file(path);
}

pub fn trim_to_count(dir: &Path, max_files: usize) -> usize {
    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    collect_files(dir, &mut files);
    if files.len() <= max_files {
        return 0;
    }
    files.sort_by_key(|(_, _, mtime)| *mtime);
    let excess = files.len() - max_files;
    files
        .into_iter()
        .take(excess)
        .filter(|(path, _, _)| fs::remove_file(path).is_ok())
        .count()
}

fn collect_files(dir: &Path, out: &mut Vec<(PathBuf, u64, std::time::SystemTime)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_files(&path, out);
        } else if let Ok(mtime) = meta.modified() {
            out.push((path, meta.len(), mtime));
        }
    }
}
