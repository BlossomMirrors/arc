use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");

    let profile = if std::env::args().any(|a| a == "--release") {
        "release"
    } else {
        "debug"
    };

    let mut build = Command::new(env!("CARGO"));
    build
        .current_dir(&root)
        .args(["build", "-p", "arc-daemon", "-p", "arc-frontend"]);
    if profile == "release" {
        build.arg("--release");
    }
    let status = build.status().expect("failed to run cargo build");
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let bin_dir = target_dir.join(profile);

    let mut daemon = Command::new(bin_dir.join("arc-daemon"))
        .spawn()
        .expect("failed to start arc-daemon");

    std::thread::sleep(Duration::from_millis(800));

    let frontend_status = Command::new(bin_dir.join("arc-frontend")).status();

    let _ = daemon.kill();
    let _ = daemon.wait();

    match frontend_status {
        Ok(s) => std::process::exit(s.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("failed to start arc-frontend: {e}");
            std::process::exit(1);
        }
    }
}
