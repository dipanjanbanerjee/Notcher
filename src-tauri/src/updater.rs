//! Installs a newer Notcher from GitHub Releases. The web side finds the release;
//! this downloads its installer, runs it silently once Notcher has quit, and starts
//! the new version.

/// Only installers from the project's own releases are accepted.
const RELEASES: &str = "https://github.com/dipanjanbanerjee/notcher/releases/download/";

/// Whether `url` points at an installer in the project's releases.
fn allowed(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with(RELEASES)
        && lower.ends_with(".exe")
        && !lower.contains("..")
        && !lower.contains(['"', '\'', '&', '|', '<', '>', '^', '%', ' '])
}

/// A plain version like 1.2.3, safe to put in a file name.
fn safe_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 20
        && version.chars().all(|c| c.is_ascii_digit() || c == '.')
}

#[tauri::command]
pub async fn install_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    url: String,
    version: String,
) -> Result<(), String> {
    if !allowed(&url) || !safe_version(&version) {
        return Err("That doesn't look like a Notcher release".into());
    }
    let installer = std::env::temp_dir().join(format!("Notcher-Setup-{version}.exe"));
    let target = installer.clone();
    tauri::async_runtime::spawn_blocking(move || platform::download(&url, &target))
        .await
        .map_err(|e| e.to_string())??;
    platform::run_after_exit(&installer)?;
    log::info!("installing Notcher {version}");
    app.exit(0);
    Ok(())
}

/// Clears Windows' icon cache once after an update, so shortcuts show the new logo.
pub fn refresh_icons_once(data_dir: &std::path::Path) {
    let marker = data_dir.join("icons-version");
    let version = env!("CARGO_PKG_VERSION");
    if std::fs::read_to_string(&marker).ok().as_deref() == Some(version) {
        return;
    }
    platform::refresh_icon_cache();
    let _ = std::fs::write(marker, version);
}

#[cfg(windows)]
mod platform {
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use windows::core::HSTRING;
    use windows::Win32::System::Com::Urlmon::URLDownloadToFileW;
    use windows::Win32::UI::Shell::{
        SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_FLUSH, SHCNF_IDLIST,
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn download(url: &str, to: &Path) -> Result<(), String> {
        let _ = std::fs::remove_file(to);
        unsafe {
            URLDownloadToFileW(
                None,
                &HSTRING::from(url),
                &HSTRING::from(to.as_os_str()),
                0,
                None,
            )
        }
        .map_err(|e| format!("Download failed: {}", e.message()))?;
        let size = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
        if size < 1_000_000 {
            return Err("The download was incomplete".into());
        }
        Ok(())
    }

    /// Waits for this process to quit, installs silently, then starts the new version.
    pub fn run_after_exit(installer: &Path) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let pid = std::process::id();
        let script = format!(
            "$ErrorActionPreference='SilentlyContinue'; Wait-Process -Id {pid} -Timeout 20; \
             Start-Process -FilePath '{}' -ArgumentList '/S' -Wait; Start-Process -FilePath '{}'",
            quote(installer),
            quote(&exe),
        );
        std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Couldn't start the installer: {e}"))
    }

    /// A path inside a single-quoted PowerShell string.
    fn quote(path: &Path) -> String {
        path.display().to_string().replace('\'', "''")
    }

    pub fn refresh_icon_cache() {
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST | SHCNF_FLUSH, None, None) };
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    pub fn download(_url: &str, _to: &Path) -> Result<(), String> {
        Err("Updates are installed on Windows only".into())
    }
    pub fn run_after_exit(_installer: &Path) -> Result<(), String> {
        Err("Updates are installed on Windows only".into())
    }
    pub fn refresh_icon_cache() {}
}

#[cfg(test)]
mod tests {
    use super::{allowed, safe_version};

    #[test]
    fn accepts_only_project_installers() {
        assert!(allowed("https://github.com/dipanjanbanerjee/Notcher/releases/download/v1.0.1/Notcher-Setup.exe"));
        assert!(!allowed(
            "https://github.com/someone/else/releases/download/v1/x.exe"
        ));
        assert!(!allowed(
            "https://github.com/dipanjanbanerjee/notcher/releases/download/v1/x.msi"
        ));
        assert!(!allowed(
            "https://github.com/dipanjanbanerjee/notcher/releases/download/../x.exe"
        ));
        assert!(!allowed(
            "https://github.com/dipanjanbanerjee/notcher/releases/download/v1/a'b.exe"
        ));
        assert!(safe_version("1.0.1"));
        assert!(!safe_version("1.0.1; rm"));
        assert!(!safe_version(""));
    }
}
