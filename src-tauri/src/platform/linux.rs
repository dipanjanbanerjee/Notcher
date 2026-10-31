//! Linux: X11 and Wayland.
//!
//! * KDE Plasma, Hyprland, Sway and other compositors with `wlr-layer-shell` get a
//!   native layer-shell surface anchored to the top edge.
//! * GNOME on Wayland has no layer-shell, so Notcher runs through XWayland there,
//!   where always-on-top windows and positioning work.
//! * X11 sessions use a regular always-on-top, undecorated window.
//!
//! Click-through is done with an input shape on the top-level GDK window, which works
//! on both X11 and Wayland, so the web view receives normal enter/leave events.

use super::PlatformMode;
use crate::island::{
    fullscreen_on_monitor, HitRegion, MonitorRect, StackedWindow, WINDOW_HEIGHT, WINDOW_WIDTH,
};
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use tauri::{Runtime, WebviewWindow};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
use x11rb::rust_connection::RustConnection;

thread_local! {
    /// The layer-shell window that hosts the web view on Wayland (GTK objects are main-thread only).
    static LAYER_WINDOW: RefCell<Option<gtk::Window>> = const { RefCell::new(None) };
}

/// Picks the GDK backend before GTK starts.
pub fn before_gtk_init() {
    if std::env::var_os("GDK_BACKEND").is_some() {
        return;
    }
    if let Ok(backend) = std::env::var("NOTCHER_BACKEND") {
        std::env::set_var("GDK_BACKEND", backend);
        return;
    }
    let wayland =
        std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v.eq_ignore_ascii_case("wayland"));
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if wayland && needs_xwayland(&desktop) {
        std::env::set_var("GDK_BACKEND", "x11");
    }
}

/// Desktops whose Wayland compositor lacks layer-shell support.
fn needs_xwayland(current_desktop: &str) -> bool {
    current_desktop.split(':').any(|d| {
        matches!(
            d.to_ascii_lowercase().as_str(),
            "gnome" | "ubuntu" | "pop" | "unity"
        )
    })
}

fn on_wayland() -> bool {
    gtk::gdk::Display::default()
        .map(|d| d.type_().name().contains("Wayland"))
        .unwrap_or(false)
}

pub fn prepare_island<R: Runtime>(window: &WebviewWindow<R>) -> PlatformMode {
    let Ok(app_window) = window.gtk_window() else {
        return PlatformMode::default();
    };
    app_window.set_skip_pager_hint(true);
    app_window.set_skip_taskbar_hint(true);
    app_window.set_keep_above(true);

    if !(on_wayland() && gtk_layer_shell::is_supported()) {
        return PlatformMode::default();
    }

    // A layer surface must be configured before it is first shown, and Tauri has
    // already realized its window, so move the web view into a fresh layer window.
    let Ok(vbox) = window.default_vbox() else {
        return PlatformMode::default();
    };
    app_window.remove(&vbox);

    let layer = gtk::Window::new(gtk::WindowType::Toplevel);
    if let Some(visual) = GtkWindowExt::screen(&layer).and_then(|s| s.rgba_visual()) {
        layer.set_visual(Some(&visual));
    }
    layer.set_app_paintable(true);
    layer.set_decorated(false);
    layer.set_size_request(WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32);
    layer.init_layer_shell();
    layer.set_namespace("notcher");
    layer.set_layer(Layer::Top);
    layer.set_anchor(Edge::Top, true);
    layer.set_keyboard_mode(KeyboardMode::OnDemand);
    layer.add(&vbox);
    layer.show_all();

    LAYER_WINDOW.with(|w| *w.borrow_mut() = Some(layer));
    PlatformMode { layer_shell: true }
}

pub fn apply_hit_region<R: Runtime>(window: &WebviewWindow<R>, region: HitRegion) {
    let window = window.clone();
    let _ = window.clone().run_on_main_thread(move || {
        let target: Option<gtk::Window> = LAYER_WINDOW
            .with(|w| w.borrow().clone())
            .or_else(|| window.gtk_window().ok().map(|w| w.upcast()));
        let Some(target) = target else { return };
        if target.window().is_some() {
            set_input_shape(&target, region);
        } else {
            // Not on screen yet: apply as soon as GTK creates the native window.
            target.connect_realize(move |w| set_input_shape(w, region));
        }
    });
}

fn set_input_shape(window: &gtk::Window, region: HitRegion) {
    let Some(gdk_window) = window.window() else {
        return;
    };
    let rect = gtk::cairo::RectangleInt::new(
        region.x.floor() as i32,
        region.y.floor() as i32,
        region.width.ceil() as i32,
        region.height.ceil() as i32,
    );
    gdk_window.input_shape_combine_region(&gtk::cairo::Region::create_rectangle(&rect), 0, 0);
}

/// The input shape lets GTK deliver real enter/leave events, so no cursor polling.
pub const POLLS_POINTER: bool = false;

/// Pointer tracking is not needed on Linux: the input shape lets GTK deliver
/// real enter/leave events to the web view.
#[derive(Default)]
pub struct PointerTracker {
    _private: (),
}

impl PointerTracker {
    pub fn update<R: Runtime>(&mut self, _window: &WebviewWindow<R>, _region: HitRegion) {}
    pub fn invalidate(&mut self) {}
}

struct X11 {
    conn: RustConnection,
    root: u32,
    atoms: Atoms,
}

struct Atoms {
    client_list_stacking: u32,
    current_desktop: u32,
    wm_desktop: u32,
    wm_state: u32,
    fullscreen: u32,
    hidden: u32,
    window_type: u32,
    /// Window types that are part of the desktop shell rather than apps.
    shell_types: Vec<u32>,
}

thread_local! {
    static X11_CONN: RefCell<Option<X11>> = const { RefCell::new(None) };
}

impl X11 {
    fn connect() -> Option<Self> {
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let atom = |name: &str| -> Option<u32> {
            Some(
                conn.intern_atom(false, name.as_bytes())
                    .ok()?
                    .reply()
                    .ok()?
                    .atom,
            )
        };
        let shell_types = [
            "_NET_WM_WINDOW_TYPE_DESKTOP",
            "_NET_WM_WINDOW_TYPE_DOCK",
            "_NET_WM_WINDOW_TYPE_NOTIFICATION",
            "_NET_WM_WINDOW_TYPE_TOOLTIP",
            "_NET_WM_WINDOW_TYPE_DND",
        ]
        .iter()
        .filter_map(|n| atom(n))
        .collect();
        let atoms = Atoms {
            client_list_stacking: atom("_NET_CLIENT_LIST_STACKING")?,
            current_desktop: atom("_NET_CURRENT_DESKTOP")?,
            wm_desktop: atom("_NET_WM_DESKTOP")?,
            wm_state: atom("_NET_WM_STATE")?,
            fullscreen: atom("_NET_WM_STATE_FULLSCREEN")?,
            hidden: atom("_NET_WM_STATE_HIDDEN")?,
            window_type: atom("_NET_WM_WINDOW_TYPE")?,
            shell_types,
        };
        Some(Self { conn, root, atoms })
    }

    fn cursor(&self) -> Option<(f64, f64)> {
        let p = self.conn.query_pointer(self.root).ok()?.reply().ok()?;
        Some((p.root_x as f64, p.root_y as f64))
    }

    fn monitors(&self, scale: f64) -> Option<Vec<MonitorRect>> {
        use x11rb::protocol::randr::ConnectionExt as _;
        let reply = self
            .conn
            .randr_get_monitors(self.root, true)
            .ok()?
            .reply()
            .ok()?;
        Some(
            reply
                .monitors
                .iter()
                .map(|m| MonitorRect {
                    x: m.x as i32,
                    y: m.y as i32,
                    width: m.width as u32,
                    height: m.height as u32,
                    scale,
                })
                .collect(),
        )
    }

    fn property32(&self, window: u32, property: u32, max_len: u32) -> Option<Vec<u32>> {
        let reply = self
            .conn
            .get_property(false, window, property, AtomEnum::ANY, 0, max_len)
            .ok()?
            .reply()
            .ok()?;
        let values: Vec<u32> = reply.value32()?.collect();
        Some(values)
    }

    /// App windows from top to bottom, excluding `own`, hidden and shell windows, and
    /// windows on other workspaces. Stops early once `enough` says so.
    fn stacked_windows(
        &self,
        own: u32,
        mut enough: impl FnMut(&StackedWindow) -> bool,
    ) -> Option<Vec<StackedWindow>> {
        let a = &self.atoms;
        let stacking = self.property32(self.root, a.client_list_stacking, 4096)?;
        let current_desktop = self
            .property32(self.root, a.current_desktop, 1)
            .and_then(|v| v.first().copied());
        let mut out = Vec::new();
        for &w in stacking.iter().rev() {
            if w == own {
                continue;
            }
            if let (Some(current), Some(desktop)) = (
                current_desktop,
                self.property32(w, a.wm_desktop, 1)
                    .and_then(|v| v.first().copied()),
            ) {
                if desktop != current && desktop != 0xFFFF_FFFF {
                    continue;
                }
            }
            let types = self.property32(w, a.window_type, 16).unwrap_or_default();
            if types.iter().any(|t| a.shell_types.contains(t)) {
                continue;
            }
            let state = self.property32(w, a.wm_state, 64).unwrap_or_default();
            if state.contains(&a.hidden) {
                continue;
            }
            let Some(geometry) = self.conn.get_geometry(w).ok().and_then(|c| c.reply().ok()) else {
                continue;
            };
            let Some(origin) = self
                .conn
                .translate_coordinates(w, self.root, 0, 0)
                .ok()
                .and_then(|c| c.reply().ok())
            else {
                continue;
            };
            let info = StackedWindow {
                x: origin.dst_x as i32,
                y: origin.dst_y as i32,
                width: geometry.width as u32,
                height: geometry.height as u32,
                fullscreen: state.contains(&a.fullscreen),
            };
            out.push(info);
            if enough(&info) {
                break;
            }
        }
        Some(out)
    }
}

fn x11_window_id<R: Runtime>(window: &WebviewWindow<R>) -> Option<u32> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(h) => Some(h.window as u32),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        _ => None,
    }
}

/// Runs `f` with this thread's X11 connection, (re)connecting as needed.
fn with_x11<T>(f: impl FnOnce(&X11) -> Option<T>) -> Option<T> {
    X11_CONN.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            *cell = X11::connect();
        }
        let result = cell.as_ref().and_then(f);
        if result.is_none() && cell.as_ref().is_some_and(|x| x.conn.flush().is_err()) {
            // The connection broke (e.g. X server restart); reconnect next time.
            *cell = None;
        }
        result
    })
}

pub fn cursor_position<R: Runtime>(_window: &WebviewWindow<R>) -> Option<(f64, f64)> {
    with_x11(X11::cursor)
}

pub fn monitors<R: Runtime>(window: &WebviewWindow<R>) -> Vec<MonitorRect> {
    // GDK applies one scale to the whole X11 screen; read it once.
    thread_local! {
        static SCALE: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
    }
    let scale = SCALE.with(|s| {
        if s.get().is_none() {
            s.set(window.scale_factor().ok());
        }
        s.get().unwrap_or(1.0)
    });
    with_x11(|x| x.monitors(scale))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| {
            window
                .available_monitors()
                .map(|ms| ms.iter().map(super::to_rect).collect())
                .unwrap_or_default()
        })
}

/// True when the top-most app window on `monitor` is full-screen (X11 and XWayland;
/// layer-shell compositors already keep the island under full-screen windows).
pub fn fullscreen_app_active<R: Runtime>(window: &WebviewWindow<R>, monitor: &MonitorRect) -> bool {
    thread_local! {
        static OWN: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
    }
    let own = OWN.with(|o| {
        if o.get().is_none() {
            o.set(x11_window_id(window));
        }
        o.get().unwrap_or(0)
    });
    with_x11(|x| {
        // Only the top-most window on this monitor matters, so stop there.
        x.stacked_windows(own, |w| {
            monitor.contains(
                w.x as f64 + w.width as f64 / 2.0,
                w.y as f64 + w.height as f64 / 2.0,
            )
        })
    })
    .is_some_and(|w| fullscreen_on_monitor(&w, monitor))
}

#[cfg(test)]
mod tests {
    use super::needs_xwayland;

    #[test]
    fn gnome_family_desktops_use_xwayland() {
        assert!(needs_xwayland("GNOME"));
        assert!(needs_xwayland("ubuntu:GNOME"));
        assert!(needs_xwayland("pop:GNOME"));
        assert!(!needs_xwayland("KDE"));
        assert!(!needs_xwayland("Hyprland"));
        assert!(!needs_xwayland("sway"));
        assert!(!needs_xwayland(""));
    }
}
