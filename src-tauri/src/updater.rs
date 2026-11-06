//! Installs a newer Notcher from GitHub Releases. The web side finds the release;
//! this downloads its installer, runs it silently once Notcher has quit, and starts
//! the new version.
//!
//! An update always replaces the installed copy in place, never next to it:
//! * A copy installed with Notcher-Setup.exe (NSIS, per user, with `uninstall.exe`
//!   beside it) is updated by the new Setup.exe run with `/S /UPDATE`. NSIS reads the
//!   old install folder from the registry and overwrites it; `/UPDATE` keeps the
//!   user's shortcuts and launch-at-login as they are.
//! * A copy installed from the .msi is updated by the new .msi (`msiexec /i`), which
//!   Windows Installer treats as a major upgrade of the same product and swaps in
//!   place. Running the NSIS setup there would add a second copy.
//!
//! The downloaded installer is deleted once it has run, and leftovers from older
//! updates are swept from the temp folder at start-up. User data (settings, the
//! database, screen time, Drop files) lives outside the install folder and is never
//! touched by an update.

/// Only installers from the project's own releases are accepted.
const RELEASES: &str = "https://github.com/dipanjanbanerjee/notcher/releases/download/";

/// Whether `url` points at an installer (`ext`, ".exe" or ".msi") in the project's
/// releases.
fn allowed(url: &str, ext: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with(RELEASES)
        && lower.ends_with(ext)
        && !lower.contains("..")
        && !lower.contains(['"', '\'', '&', '|', '<', '>', '^', '%', ' '])
}

/// A plain version like 1.2.3, safe to put in a file name.
fn safe_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 20
        && version.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Name of a downloaded installer in the temp folder. Kept in one place so the
/// start-up sweep finds exactly these files.
fn installer_name(version: &str, msi: bool) -> String {
    if msi {
        format!("Notcher-Update-{version}.msi")
    } else {
        format!("Notcher-Setup-{version}.exe")
    }
}

/// Whether a file in the temp folder is an installer an update downloaded.
fn is_downloaded_installer(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.starts_with("notcher-setup-") && lower.ends_with(".exe"))
        || (lower.starts_with("notcher-update-") && lower.ends_with(".msi"))
}

#[tauri::command]
pub async fn install_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    url: String,
    version: String,
    msi_url: Option<String>,
) -> Result<(), String> {
    if !allowed(&url, ".exe") || !safe_version(&version) {
        return Err("That doesn't look like a Notcher release".into());
    }
    // Update the copy the way it was installed, so it is replaced, not doubled.
    let from_msi = platform::installed_from_msi();
    let url = if from_msi {
        match msi_url.filter(|u| allowed(u, ".msi")) {
            Some(u) => u,
            None => {
                return Err(
                    "This copy was installed from the .msi, and the release has no .msi to update it with".into(),
                )
            }
        }
    } else {
        url
    };
    let installer = std::env::temp_dir().join(installer_name(&version, from_msi));
    let target = installer.clone();
    tauri::async_runtime::spawn_blocking(move || platform::download(&url, &target))
        .await
        .map_err(|e| e.to_string())??;
    platform::run_after_exit(&installer, from_msi)?;
    log::info!(
        "installing Notcher {version} from the {}",
        if from_msi { "MSI" } else { "setup" }
    );
    app.exit(0);
    Ok(())
}

/// Deletes installers that earlier updates downloaded to the temp folder (older
/// versions of Notcher left them there).
pub fn clean_old_installers() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_downloaded_installer(&name) && std::fs::remove_file(entry.path()).is_ok() {
            log::info!("update: removed the old installer {name}");
        }
    }
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

    /// Notcher-Setup.exe puts `uninstall.exe` next to Notcher.exe; the MSI doesn't.
    /// A development build (in a `target` folder) counts as a setup install, which
    /// keeps "install update" from ever running msiexec there.
    pub fn installed_from_msi() -> bool {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let Some(dir) = exe.parent() else {
            return false;
        };
        let dev = dir.components().any(|c| c.as_os_str() == "target");
        !dev && !dir.join("uninstall.exe").exists()
    }
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

    /// Waits for this process to quit, installs over the current copy, deletes the
    /// installer, then starts the new version.
    pub fn run_after_exit(installer: &Path, msi: bool) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let pid = std::process::id();
        let install = if msi {
            // A major upgrade of the installed product; Windows may ask for admin
            // rights, since the MSI installs for all users.
            format!(
                "Start-Process -FilePath msiexec.exe -ArgumentList @('/i', '\"{}\"', '/passive', '/norestart') -Wait",
                quote(installer)
            )
        } else {
            // /UPDATE: replace the files in the existing install folder and keep the
            // user's shortcuts and launch-at-login settings as they are.
            format!(
                "Start-Process -FilePath '{}' -ArgumentList @('/S', '/UPDATE') -Wait",
                quote(installer)
            )
        };
        let script = format!(
            "$ErrorActionPreference='SilentlyContinue'; Wait-Process -Id {pid} -Timeout 20; \
             {install}; Remove-Item -LiteralPath '{}' -Force; Start-Process -FilePath '{}'",
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
    pub fn installed_from_msi() -> bool {
        false
    }
    pub fn run_after_exit(_installer: &Path, _msi: bool) -> Result<(), String> {
        Err("Updates are installed on Windows only".into())
    }
    pub fn refresh_icon_cache() {}
}

#[cfg(test)]
mod tests {
    use super::{allowed, installer_name, is_downloaded_installer, safe_version};

    #[test]
    fn accepts_only_project_installers() {
        assert!(allowed(
            "https://github.com/dipanjanbanerjee/Notcher/releases/download/v1.0.1/Notcher-Setup.exe",
            ".exe"
        ));
        assert!(!allowed(
            "https://github.com/someone/else/releases/download/v1/x.exe",
            ".exe"
        ));
        assert!(!allowed(
            "https://github.com/dipanjanbanerjee/notcher/releases/download/v1/x.msi",
            ".exe"
        ));
        assert!(allowed(
            "https://github.com/dipanjanbanerjee/notcher/releases/download/v1.0.3/Notcher_1.0.3_x64_en-US.msi",
            ".msi"
        ));
        assert!(!allowed(
            "https://github.com/dipanjanbanerjee/notcher/releases/download/../x.exe",
            ".exe"
        ));
        assert!(!allowed(
            "https://github.com/dipanjanbanerjee/notcher/releases/download/v1/a'b.exe",
            ".exe"
        ));
        assert!(safe_version("1.0.1"));
        assert!(!safe_version("1.0.1; rm"));
        assert!(!safe_version(""));
    }

    #[test]
    fn the_sweep_finds_downloaded_installers_and_nothing_else() {
        for version in ["1.0.1", "1.0.3"] {
            assert!(is_downloaded_installer(&installer_name(version, false)));
            assert!(is_downloaded_installer(&installer_name(version, true)));
        }
        // What 1.0.1 and 1.0.2 left behind.
        assert!(is_downloaded_installer("Notcher-Setup-1.0.2.exe"));
        // Never the user's own downloads or other apps' files.
        assert!(!is_downloaded_installer("Notcher-Setup.exe.part"));
        assert!(!is_downloaded_installer("Notcher.exe"));
        assert!(!is_downloaded_installer("notcher.db"));
        assert!(!is_downloaded_installer("Other-Setup-1.0.exe"));
    }
}
