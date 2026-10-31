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
use crate::island::{HitRegion, WINDOW_HEIGHT, WINDOW_WIDTH};
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
    active_atom: u32,
    state_atom: u32,
    fullscreen_atom: u32,
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
        let active_atom = atom("_NET_ACTIVE_WINDOW")?;
        let state_atom = atom("_NET_WM_STATE")?;
        let fullscreen_atom = atom("_NET_WM_STATE_FULLSCREEN")?;
        Some(Self {
            conn,
            root,
            active_atom,
            state_atom,
            fullscreen_atom,
        })
    }

    /// Bounds (x, y, width, height) of the active window if it is full-screen.
    fn fullscreen_window(&self) -> Option<(i32, i32, u32, u32)> {
        let active = self
            .conn
            .get_property(false, self.root, self.active_atom, AtomEnum::ANY, 0, 1)
            .ok()?
            .reply()
            .ok()?
            .value32()?
            .next()
            .filter(|w| *w != 0)?;
        let is_fullscreen = self
            .conn
            .get_property(false, active, self.state_atom, AtomEnum::ANY, 0, 64)
            .ok()?
            .reply()
            .ok()?
            .value32()?
            .any(|a| a == self.fullscreen_atom);
        if !is_fullscreen {
            return None;
        }
        let geometry = self.conn.get_geometry(active).ok()?.reply().ok()?;
        let origin = self
            .conn
            .translate_coordinates(active, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        Some((
            origin.dst_x as i32,
            origin.dst_y as i32,
            geometry.width as u32,
            geometry.height as u32,
        ))
    }
}

/// True when the focused window is full-screen on the island's monitor (X11/XWayland only;
/// layer-shell compositors already keep the island under full-screen windows).
pub fn fullscreen_app_active<R: Runtime>(window: &WebviewWindow<R>) -> bool {
    let Some((x, y, w, h)) = X11_CONN.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            *cell = X11::connect();
        }
        let result = cell.as_ref().and_then(X11::fullscreen_window);
        if result.is_none() && cell.as_ref().is_some_and(|c| c.conn.flush().is_err()) {
            // Connection broke (e.g. X server restart); reconnect next time.
            *cell = None;
        }
        result
    }) else {
        return false;
    };
    let Ok(Some(monitor)) = window.current_monitor() else {
        return true;
    };
    let centre_x = x as f64 + w as f64 / 2.0;
    let centre_y = y as f64 + h as f64 / 2.0;
    let pos = monitor.position();
    let size = monitor.size();
    centre_x >= pos.x as f64
        && centre_x < pos.x as f64 + size.width as f64
        && centre_y >= pos.y as f64
        && centre_y < pos.y as f64 + size.height as f64
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
