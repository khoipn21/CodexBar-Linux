use anyhow::{Context, Result};
use std::path::PathBuf;

const DESKTOP_FILE: &str = "codexbar-tray.desktop";

pub fn is_enabled() -> bool {
    autostart_path().exists()
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let path = autostart_path();
    if enabled {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).context("creating autostart dir")?;
        }
        std::fs::write(&path, desktop_entry()?).context("writing autostart desktop entry")?;
    } else if path.exists() {
        std::fs::remove_file(&path).context("removing autostart desktop entry")?;
    }
    Ok(())
}

fn autostart_path() -> PathBuf {
    config_home().join("autostart").join(DESKTOP_FILE)
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn desktop_entry() -> Result<String> {
    let exe = std::env::current_exe().context("locating current executable")?;
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=CodexBar\nComment=AI coding-provider usage in your panel\nExec={}\nIcon=codexbar\nTerminal=false\nCategories=Utility;Development;\nX-GNOME-Autostart-enabled=true\n",
        shell_escape(&exe.to_string_lossy())
    ))
}

fn shell_escape(raw: &str) -> String {
    if raw.bytes().all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b)) {
        raw.to_string()
    } else {
        format!("'{}'", raw.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_escape_keeps_simple_paths() {
        assert_eq!(shell_escape("/opt/codexbar/codexbar-tray"), "/opt/codexbar/codexbar-tray");
    }

    #[test]
    fn shell_escape_quotes_spaces() {
        assert_eq!(shell_escape("/tmp/a b"), "'/tmp/a b'");
    }
}
