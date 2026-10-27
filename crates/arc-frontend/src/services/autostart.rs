use std::fs;
use std::io;
use std::path::PathBuf;

const ENTRY: &str = "arc-frontend-standby.desktop";
const SYSTEM_DIR: &str = "/etc/xdg/autostart";

fn user_entry() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("autostart").join(ENTRY))
}

fn is_hidden(content: &str) -> bool {
    content.lines().any(|l| l.trim().eq_ignore_ascii_case("Hidden=true"))
}

pub fn is_enabled() -> bool {
    // a file with the same name in the user folder overrides the system wide entry
    if let Some(content) = user_entry().and_then(|p| fs::read_to_string(p).ok()) {
        return !is_hidden(&content);
    }
    PathBuf::from(SYSTEM_DIR).join(ENTRY).exists()
}

pub fn set_enabled(enabled: bool) -> io::Result<()> {
    let path = user_entry().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config folder"))?;
    let system_entry = PathBuf::from(SYSTEM_DIR).join(ENTRY).exists();
    // without a user file the system entry decides so it only needs one when the wish differs
    if enabled == system_entry {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let content = if enabled {
        let exe = std::env::current_exe()?;
        format!(
            "[Desktop Entry]\nType=Application\nName=Arc Store Standby\nExec={} --standby\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        )
    } else {
        "[Desktop Entry]\nType=Application\nName=Arc Store Standby\nHidden=true\n".to_string()
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, content)
}
