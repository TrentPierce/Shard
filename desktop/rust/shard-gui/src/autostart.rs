/// Register or unregister the app for system autostart.
pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    return windows::set_autostart(enabled);

    #[cfg(target_os = "macos")]
    return macos::set_autostart(enabled);

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    return linux::set_autostart(enabled);
}

/// Check whether autostart is currently registered.
pub fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    return windows::is_autostart_enabled();

    #[cfg(target_os = "macos")]
    return macos::is_autostart_enabled();

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    return linux::is_autostart_enabled();
}

// ─── Windows ────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows {
    use winreg::enums::*;
    use winreg::RegKey;

    const APP_NAME: &str = "ShardDesktop";

    fn run_key() -> anyhow::Result<RegKey> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
        Ok(hkcu.open_subkey_with_flags(path, KEY_ALL_ACCESS)?)
    }

    pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
        let key = run_key()?;
        if enabled {
            let exe = std::env::current_exe()?.display().to_string();
            key.set_value(APP_NAME, &exe)?;
        } else {
            let _ = key.delete_value(APP_NAME);
        }
        Ok(())
    }

    pub fn is_autostart_enabled() -> bool {
        run_key()
            .ok()
            .and_then(|k| k.get_value::<String, _>(APP_NAME).ok())
            .is_some()
    }
}

// ─── macOS ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    use std::path::PathBuf;

    const PLIST_NAME: &str = "com.shard.desktop.plist";

    fn plist_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join("Library/LaunchAgents").join(PLIST_NAME))
    }

    pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
        let path = plist_path().ok_or_else(|| anyhow::anyhow!("cannot find home dir"))?;
        if enabled {
            let exe = std::env::current_exe()?.display().to_string();
            std::fs::create_dir_all(path.parent().unwrap())?;
            let plist = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.shard.desktop</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
</dict>
</plist>
"#
            );
            std::fs::write(&path, plist)?;
        } else if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn is_autostart_enabled() -> bool {
        plist_path().map(|p| p.exists()).unwrap_or(false)
    }
}

// ─── Linux ──────────────────────────────────────────────────────────────────

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
mod linux {
    use std::path::PathBuf;

    const DESKTOP_NAME: &str = "shard-desktop.desktop";

    fn desktop_path() -> Option<PathBuf> {
        dirs::config_dir().map(|c| c.join("autostart").join(DESKTOP_NAME))
    }

    pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
        let path = desktop_path().ok_or_else(|| anyhow::anyhow!("cannot find config dir"))?;
        if enabled {
            let exe = std::env::current_exe()?.display().to_string();
            std::fs::create_dir_all(path.parent().unwrap())?;
            let entry = format!(
                "[Desktop Entry]\nType=Application\nName=Shard Desktop\nExec={exe}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n"
            );
            std::fs::write(&path, entry)?;
        } else if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn is_autostart_enabled() -> bool {
        desktop_path().map(|p| p.exists()).unwrap_or(false)
    }
}
