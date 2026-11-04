//! Dropping files on the island (Windows).
//!
//! Tauri puts its drop handler on the web view's windows once, right when the web view
//! is created. WebView2 can create its inner windows a moment later (on slower PCs) or
//! create them again (after its GPU process restarts, after sleep, or after a display
//! change). Those windows only have Chromium's own handler, which turns files from
//! Explorer away, so dragging files onto the island worked on some PCs and not others.
//!
//! This module puts its own handler on every window inside the island, again whenever
//! new ones appear, and writes each drag to the log so a failure can be traced from the
//! log folder. It also notices when Notcher runs as administrator, which Windows
//! blocks drops into from a normal Explorer window.

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::rc::Rc;
use tauri::{AppHandle, Emitter, Runtime};
use windows::core::{implement, Ref, BOOL};
use windows::Win32::{
    Foundation::{HANDLE, HWND, LPARAM, POINTL},
    Security::{GetTokenInformation, TokenElevationType, TOKEN_ELEVATION_TYPE, TOKEN_QUERY},
    System::{
        Com::{IDataObject, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL},
        Ole::{
            IDropTarget, IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
            CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE,
        },
        SystemServices::MODIFIERKEYS_FLAGS,
        Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcessToken},
    },
    UI::{
        Shell::{DragQueryFileW, HDROP},
        WindowsAndMessaging::{
            EnumChildWindows, GetClassNameW, GetWindowThreadProcessId, IsWindow,
        },
    },
};

/// Sent to the island for each step of a file drag: `{ kind, paths }`, where kind is
/// "enter", "over", "leave" or "drop".
pub const FILE_DRAG_EVENT: &str = "island://file-drag";

#[derive(serde::Serialize, Clone)]
struct FileDrag {
    kind: &'static str,
    paths: Vec<String>,
}

type Listener = Rc<dyn Fn(&'static str, Vec<String>)>;

thread_local! {
    /// Windows that already have our handler. Only touched on the main thread.
    static REGISTERED: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
    /// Windows already reported as belonging to another process.
    static FOREIGN: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
}

/// Every window inside `parent`, at any depth.
pub fn descendants(parent: HWND) -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
        let out = &mut *(data.0 as *mut Vec<HWND>);
        out.push(hwnd);
        BOOL(1)
    }
    let mut out: Vec<HWND> = Vec::new();
    unsafe {
        let _ = EnumChildWindows(
            Some(parent),
            Some(collect),
            LPARAM(&mut out as *mut Vec<HWND> as isize),
        );
    }
    out
}

/// A cheap summary of the island's inner windows, to notice when WebView2 adds or
/// replaces one. Safe to call from any thread.
pub fn fingerprint(island: HWND) -> u64 {
    descendants(island)
        .iter()
        .fold(0u64, |acc, h| acc.rotate_left(7) ^ (h.0 as usize as u64))
}

fn class_of(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
    String::from_utf16_lossy(&buf[..len.min(buf.len())])
}

/// Puts our drop handler on the island and every window inside it that doesn't have
/// it yet. Must run on the main thread, which owns those windows.
pub fn ensure_targets<R: Runtime>(app: &AppHandle<R>, island: HWND) {
    let app = app.clone();
    let listener: Listener = Rc::new(move |kind, paths| {
        let _ = app.emit_to("island", FILE_DRAG_EVENT, FileDrag { kind, paths });
    });
    let own_pid = unsafe { GetCurrentProcessId() };
    let mut windows = vec![island];
    windows.extend(descendants(island));
    REGISTERED.with(|set| {
        let mut set = set.borrow_mut();
        set.retain(|h| unsafe { IsWindow(Some(HWND(*h as *mut _))) }.as_bool());
        for hwnd in windows {
            let key = hwnd.0 as isize;
            if set.contains(&key) {
                continue;
            }
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            if pid != own_pid {
                FOREIGN.with(|f| {
                    if f.borrow_mut().insert(key) {
                        log::info!(
                            "file drop: {} belongs to another process; drops over it go to its parent",
                            class_of(hwnd)
                        );
                    }
                });
                continue;
            }
            let target: IDropTarget = DropTarget {
                listener: listener.clone(),
                has_files: RefCell::new(false),
            }
            .into();
            // Replaces Tauri's handler, or Chromium's, which refuses outside files.
            let _ = unsafe { RevokeDragDrop(hwnd) };
            match unsafe { RegisterDragDrop(hwnd, &target) } {
                Ok(()) => {
                    set.insert(key);
                    log::info!("file drop: ready on {}", class_of(hwnd));
                }
                Err(err) => log::warn!(
                    "file drop: could not listen on {}: {err}",
                    class_of(hwnd)
                ),
            }
        }
    });
}

#[implement(IDropTarget)]
struct DropTarget {
    listener: Listener,
    /// The drag in progress carries files.
    has_files: RefCell<bool>,
}

/// The file paths in a drag, or `None` when it carries no files (text, a link…).
fn paths_of(data: Option<&IDataObject>) -> Option<Vec<String>> {
    let data = data?;
    let format = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    unsafe {
        let mut medium = data.GetData(&format).ok()?;
        let hdrop = HDROP(medium.u.hGlobal.0 as _);
        let count = DragQueryFileW(hdrop, u32::MAX, None);
        let mut paths = Vec::with_capacity(count as usize);
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, None) as usize;
            let mut buf = vec![0u16; len + 1];
            DragQueryFileW(hdrop, i, Some(&mut buf));
            paths.push(
                OsString::from_wide(&buf[..len])
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        ReleaseStgMedium(&mut medium);
        Some(paths)
    }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let paths = paths_of(data.as_ref());
        let files = paths.as_ref().is_some_and(|p| !p.is_empty());
        *self.has_files.borrow_mut() = files;
        if files {
            let paths = paths.unwrap_or_default();
            log::info!("file drop: {} item(s) dragged over the island", paths.len());
            (self.listener)("enter", paths);
        } else {
            log::info!("file drop: something without files was dragged over the island");
        }
        if !effect.is_null() {
            unsafe {
                *effect = if files {
                    DROPEFFECT_COPY
                } else {
                    DROPEFFECT_NONE
                }
            };
        }
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        if !effect.is_null() {
            let files = *self.has_files.borrow();
            unsafe {
                *effect = if files {
                    DROPEFFECT_COPY
                } else {
                    DROPEFFECT_NONE
                }
            };
        }
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        if self.has_files.replace(false) {
            log::info!("file drop: the drag left the island");
            (self.listener)("leave", Vec::new());
        }
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        self.has_files.replace(false);
        let paths = paths_of(data.as_ref()).unwrap_or_default();
        log::info!("file drop: {} item(s) dropped on the island", paths.len());
        if !effect.is_null() {
            let e = if paths.is_empty() {
                DROPEFFECT_NONE
            } else {
                DROPEFFECT_COPY
            };
            unsafe { *effect = e };
        }
        if !paths.is_empty() {
            (self.listener)("drop", paths);
        }
        Ok(())
    }
}

/// True when Notcher was started "as administrator" while Explorer runs normally.
/// Windows then blocks dragging files from Explorer onto it.
pub fn runs_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut kind = TOKEN_ELEVATION_TYPE::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevationType,
            Some(&mut kind as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION_TYPE>() as u32,
            &mut size,
        )
        .is_ok();
        let _ = windows::Win32::Foundation::CloseHandle(token);
        // "Full" means a split admin token that was elevated; "Default" (UAC off, or
        // the built-in Administrator) runs Explorer at the same level, so drops work.
        ok && kind == windows::Win32::Security::TokenElevationTypeFull
    }
}

/// When Notcher was started as administrator (for example from an installer that ran
/// elevated), starts it again as a normal app through Explorer so that drags from
/// Explorer reach it. Returns true when the restart is on its way and this copy should
/// quit. A marker file stops it from trying twice in a row; if Explorer doesn't start
/// it within a few seconds, the script starts it the old way.
pub fn restart_unelevated(data_dir: &std::path::Path) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if !runs_elevated() {
        return false;
    }
    let marker = data_dir.join("unelevated-restart");
    let recent = std::fs::metadata(&marker)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < 120);
    if recent {
        log::warn!(
            "Notcher is running as administrator, so Windows blocks dragging files onto it from Explorer. Start it normally to fix this."
        );
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let _ = std::fs::write(&marker, b"1");
    let name = exe
        .file_stem()
        .map(|s| s.to_string_lossy().replace('\'', "''"))
        .unwrap_or_else(|| "Notcher".into());
    let path = exe.display().to_string().replace('\'', "''");
    let pid = std::process::id();
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; Wait-Process -Id {pid} -Timeout 15; \
         Start-Process -FilePath explorer.exe -ArgumentList ('\"' + '{path}' + '\"'); Start-Sleep -Seconds 5; \
         if (-not (Get-Process -Name '{name}')) {{ Start-Process -FilePath '{path}' }}"
    );
    let spawned = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    match spawned {
        Ok(_) => {
            log::warn!("Notcher was started as administrator; restarting it normally so file drops from Explorer work");
            true
        }
        Err(err) => {
            log::warn!("Notcher runs as administrator and couldn't restart normally: {err}");
            false
        }
    }
}
