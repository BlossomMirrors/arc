use std::path::Path;

pub fn escape_desktop_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

pub fn quote_exec_arg(value: &str) -> String {
    const RESERVED: &[char; 19] = &[
        ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(',
        ')', '`',
    ];
    if !value.contains(RESERVED) {
        return value.to_string();
    }
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for c in value.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

pub struct DesktopEntry {
    pub name: String,
    pub comment: String,
    // fully-assembled Exec value (program + args); build it with quote_exec_arg
    // on any individual value that may contain shell metacharacters, render()
    // takes care of the desktop-file-level value escaping on top of that
    pub exec: String,
    pub icon: String,
    pub categories: String,
    pub version: String,
    pub start_notify: bool,
    pub startup_wm_class: Option<String>,
    pub extra: Vec<(String, String)>,
}

impl Default for DesktopEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            comment: String::new(),
            exec: String::new(),
            icon: String::new(),
            categories: String::new(),
            version: "1.0".to_string(),
            start_notify: false,
            startup_wm_class: None,
            extra: Vec::new(),
        }
    }
}

impl DesktopEntry {
    pub fn render(&self) -> String {
        let mut lines = vec![
            "[Desktop Entry]".to_string(),
            "Type=Application".to_string(),
            format!("Version={}", escape_desktop_value(&self.version)),
            format!("Name={}", escape_desktop_value(&self.name)),
            format!("Comment={}", escape_desktop_value(&self.comment.replace('\n', " "))),
            format!("Exec={}", escape_desktop_value(&self.exec)),
            format!("Icon={}", escape_desktop_value(&self.icon)),
            format!("Categories={}", escape_desktop_value(&self.categories)),
        ];
        if self.start_notify {
            lines.push("StartupNotify=true".to_string());
        }
        if let Some(ref wm_class) = self.startup_wm_class {
            lines.push(format!("StartupWMClass={}", escape_desktop_value(wm_class)));
        }
        for (key, value) in &self.extra {
            lines.push(format!("{key}={}", escape_desktop_value(value)));
        }
        lines.join("\n")
    }

    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, self.render())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(path, perms);
            }
        }
        Ok(())
    }
}
