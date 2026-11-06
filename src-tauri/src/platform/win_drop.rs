//! Dropping files on the island (Windows).
//!
//! Tauri puts its drop handler on the web view's windows once, right when the web view
//! is created. WebView2 can create its inner windows a moment later (on slower PCs) or
//! create them again (after its GPU process restarts, after sleep, or after a display
//! change). Those windows only have Chromium's own handler, which turns files from
//! Explorer away, so dragging files onto the island worked on some PCs and not others.
//!
//! On some PCs the window under the cursor isn't even Notcher's: WebView2 draws the page
//! in a window that belongs to its own msedgewebview2.exe process. Windows asks that
//! window's handler first, Chromium refuses files from outside, and the drop is lost.
//! No handler Notcher puts on its own windows can change that.
//!
//! So the island has a drop catcher: a tiny native window, invisible to the eye, with
//! no web view inside. While something is being dragged near the top of the screen, the
//! watcher lays it over the notch (and over the open panel), on top of everything.
//! Windows then hands the drag straight to Notcher's own handler, whatever WebView2
//! version, DPI or click-through state the island is in. It goes away again as soon as
//! the mouse button is released, so it never gets in the way of clicks.
//!
//! The handlers on the web view's windows stay as a second line, and every step of a
//! drag is written to the log (`file drop:` lines) so a failure can be traced from the
//! log folder. This module also notices when Notcher runs as administrator, which
//! Windows blocks drops into from a normal Explorer window.

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Runtime};
use windows::core::{implement, w, Ref, BOOL};
use windows::Win32::{
    Foundation::{COLORREF, HANDLE, HWND, LPARAM, LRESULT, POINTL, RECT, WPARAM},
    Security::{GetTokenInformation, TokenElevationType, TOKEN_ELEVATION_TYPE, TOKEN_QUERY},
    System::{
        Com::{IDataObject, DATADIR_GET, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL},
        DataExchange::GetClipboardFormatNameW,
        LibraryLoader::GetModuleHandleW,
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
            CreateWindowExW, DefWindowProcW, EnumChildWindows, GetClassNameW,
            GetWindowThreadProcessId, IsWindow, LoadCursorW, RegisterClassW,
            SetLayeredWindowAttributes, SetWindowPos, HWND_TOPMOST, IDC_ARROW, LWA_ALPHA,
            MA_NOACTIVATE, SWP_ASYNCWINDOWPOS, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, WM_MOUSEACTIVATE, WNDCLASSW, WS_EX_LAYERED,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
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

/// How many drags have reached one of Notcher's handlers. The watcher compares it
/// before and after a drag over the notch to tell when Windows delivered nothing.
static DRAG_ENTERS: AtomicU32 = AtomicU32::new(0);

/// Number of drags that have reached Notcher so far.
pub fn drag_enters() -> u32 {
    DRAG_ENTERS.load(Ordering::Relaxed)
}

fn island_listener<R: Runtime>(app: &AppHandle<R>) -> Listener {
    let app = app.clone();
    Rc::new(move |kind, paths| {
        let _ = app.emit_to("island", FILE_DRAG_EVENT, FileDrag { kind, paths });
    })
}

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
    let listener = island_listener(app);
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
            let target: IDropTarget = DropTarget::new(listener.clone(), "island").into();
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
    /// Which window this handler is on, for the log.
    place: &'static str,
    /// The drag in progress carries files.
    has_files: RefCell<bool>,
    /// Moves seen during the drag in progress, and when one was last logged.
    moves: RefCell<(u32, Option<Instant>)>,
}

impl DropTarget {
    fn new(listener: Listener, place: &'static str) -> Self {
        Self {
            listener,
            place,
            has_files: RefCell::new(false),
            moves: RefCell::new((0, None)),
        }
    }
}

/// Clipboard format names in a drag, for the log when it carries no files (a file
/// inside a zip, an Outlook attachment or a phone's photo is "virtual" and has none).
fn formats_of(data: &IDataObject) -> String {
    let mut names = Vec::new();
    unsafe {
        if let Ok(list) = data.EnumFormatEtc(DATADIR_GET.0 as u32) {
            let mut one = [FORMATETC::default()];
            let mut fetched = 0u32;
            while names.len() < 16
                && list.Next(&mut one, Some(&mut fetched)).is_ok()
                && fetched == 1
            {
                let id = one[0].cfFormat as u32;
                let mut buf = [0u16; 96];
                let len = GetClipboardFormatNameW(id, &mut buf);
                names.push(if len > 0 {
                    String::from_utf16_lossy(&buf[..len as usize])
                } else {
                    format!("#{id}")
                });
            }
        }
    }
    if names.is_empty() {
        "none listed".into()
    } else {
        names.join(", ")
    }
}

/// The file paths in a drag, or why there are none.
fn paths_of(data: Option<&IDataObject>) -> Result<Vec<String>, String> {
    let data = data.ok_or("the drag has no data")?;
    let format = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    unsafe {
        let mut medium = data.GetData(&format).map_err(|err| {
            format!(
                "it carries no files ({err}); formats offered: {}",
                formats_of(data)
            )
        })?;
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
        if paths.is_empty() {
            return Err("its file list is empty".into());
        }
        Ok(paths)
    }
}

fn set_effect(effect: *mut DROPEFFECT, files: bool) {
    if !effect.is_null() {
        unsafe {
            *effect = if files {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            }
        };
    }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        DRAG_ENTERS.fetch_add(1, Ordering::Relaxed);
        *self.moves.borrow_mut() = (0, None);
        let paths = paths_of(data.as_ref());
        let files = paths.is_ok();
        *self.has_files.borrow_mut() = files;
        match paths {
            Ok(paths) => {
                log::info!(
                    "file drop: drag-enter on the {} at ({}, {}) px with {} item(s)",
                    self.place,
                    pt.x,
                    pt.y,
                    paths.len()
                );
                (self.listener)("enter", paths);
            }
            Err(why) => log::warn!(
                "file drop: drag-enter on the {} at ({}, {}) px refused: {why}",
                self.place,
                pt.x,
                pt.y
            ),
        }
        set_effect(effect, files);
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let files = *self.has_files.borrow();
        let mut moves = self.moves.borrow_mut();
        moves.0 += 1;
        // The first move, then at most one line a second while the drag hovers.
        if !moves.1.is_some_and(|t| t.elapsed().as_secs() < 1) {
            moves.1 = Some(Instant::now());
            log::info!(
                "file drop: drag-over on the {} at ({}, {}) px, {} ({} moves)",
                self.place,
                pt.x,
                pt.y,
                if files {
                    "accepting"
                } else {
                    "refusing, no files"
                },
                moves.0
            );
        }
        set_effect(effect, files);
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        if self.has_files.replace(false) {
            log::info!(
                "file drop: drag-leave, the files went elsewhere (left the {})",
                self.place
            );
            (self.listener)("leave", Vec::new());
        }
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        self.has_files.replace(false);
        match paths_of(data.as_ref()) {
            Ok(paths) => {
                log::info!(
                    "file drop: drop on the {} at ({}, {}) px, {} item(s) received",
                    self.place,
                    pt.x,
                    pt.y,
                    paths.len()
                );
                set_effect(effect, true);
                (self.listener)("drop", paths);
            }
            Err(why) => {
                log::warn!("file drop: drop on the {} failed: {why}", self.place);
                set_effect(effect, false);
                (self.listener)("leave", Vec::new());
            }
        }
        Ok(())
    }
}

// ------------------------------------------------------------------ drop catcher

/// The drop catcher's window handle (0 until it exists).
static CATCHER: AtomicIsize = AtomicIsize::new(0);

/// The drop catcher window, once created.
pub fn catcher() -> Option<HWND> {
    match CATCHER.load(Ordering::Relaxed) {
        0 => None,
        raw => Some(HWND(raw as *mut _)),
    }
}

unsafe extern "system" fn catcher_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Never take focus away from the app the files come from.
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Creates the drop catcher, hidden. Must run on the main thread, which pumps its
/// messages and has OLE set up.
pub fn create_catcher<R: Runtime>(app: &AppHandle<R>) {
    if catcher().is_some() {
        return;
    }
    let made = unsafe {
        (|| -> windows::core::Result<HWND> {
            let instance = GetModuleHandleW(None)?;
            let class = w!("NotcherDropCatcher");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(catcher_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };
            // Zero when the class already exists, which is fine.
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                class,
                w!("Notcher drop"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            // Alpha 1 of 255: invisible to the eye, but still a real target for the
            // mouse (alpha 0 or WS_EX_TRANSPARENT would let drags fall through).
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 1, LWA_ALPHA)?;
            let target: IDropTarget = DropTarget::new(island_listener(app), "notch").into();
            RegisterDragDrop(hwnd, &target)?;
            Ok(hwnd)
        })()
    };
    match made {
        Ok(hwnd) => {
            CATCHER.store(hwnd.0 as isize, Ordering::Relaxed);
            log::info!(
                "file drop: drop catcher ready{}",
                if runs_elevated() {
                    " (but Notcher runs as administrator, so Windows may block drops from Explorer)"
                } else {
                    ""
                }
            );
        }
        Err(err) => log::warn!(
            "file drop: could not create the drop catcher ({err}); only the web view's handlers will take drops"
        ),
    }
}

/// Lays the catcher over `rect` (physical screen pixels), above every other window.
/// Safe from any thread: the move is posted to the window's own thread.
pub fn show_catcher(rect: RECT) {
    let Some(hwnd) = catcher() else { return };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.left,
            rect.top,
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_ASYNCWINDOWPOS,
        );
    }
}

/// Takes the catcher away again. Safe from any thread.
pub fn hide_catcher() {
    let Some(hwnd) = catcher() else { return };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_HIDEWINDOW
                | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOACTIVATE
                | SWP_ASYNCWINDOWPOS,
        );
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
