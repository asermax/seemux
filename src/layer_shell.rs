//! Minimal FFI bindings to libgtk4-layer-shell for Wayland layer shell support.
//!
//! We bind directly instead of using the `gtk4-layer-shell` crate due to
//! version incompatibility with our gtk4 version.

use std::ffi::c_int;

use gtk4::prelude::*;
use gtk4::ApplicationWindow;

#[allow(non_camel_case_types)]
type gboolean = c_int;

const LAYER_BOTTOM: c_int = 1;
const LAYER_TOP: c_int = 2;
const EDGE_LEFT: c_int = 0;
const EDGE_RIGHT: c_int = 1;
const EDGE_TOP: c_int = 2;
const EDGE_BOTTOM: c_int = 3;
const KEYBOARD_ON_DEMAND: c_int = 2;

#[link(name = "gtk4-layer-shell")]
unsafe extern "C" {
    fn gtk_layer_is_supported() -> gboolean;
    fn gtk_layer_init_for_window(window: *mut std::ffi::c_void);
    fn gtk_layer_set_layer(window: *mut std::ffi::c_void, layer: c_int);
    fn gtk_layer_set_keyboard_mode(window: *mut std::ffi::c_void, mode: c_int);
    fn gtk_layer_set_anchor(window: *mut std::ffi::c_void, edge: c_int, anchor: gboolean);
    fn gtk_layer_set_margin(window: *mut std::ffi::c_void, edge: c_int, margin: c_int);
    fn gtk_layer_set_respect_close(window: *mut std::ffi::c_void, respect: gboolean);
}

fn window_ptr(window: &ApplicationWindow) -> *mut std::ffi::c_void {
    use gtk4::glib::translate::ToGlibPtr;
    let ptr: *mut gtk4::ffi::GtkWindow = window.upcast_ref::<gtk4::Window>().to_glib_none().0;
    ptr as *mut std::ffi::c_void
}

pub fn is_supported() -> bool {
    unsafe { gtk_layer_is_supported() != 0 }
}

pub fn setup_dropdown(window: &ApplicationWindow, width: i32, monitor_width: i32, initial_top_margin: i32) {
    if !is_supported() {
        return;
    }

    let ptr = window_ptr(window);

    unsafe {
        gtk_layer_init_for_window(ptr);
        gtk_layer_set_layer(ptr, LAYER_TOP);
        gtk_layer_set_keyboard_mode(ptr, KEYBOARD_ON_DEMAND);
        gtk_layer_set_respect_close(ptr, 1);

        gtk_layer_set_anchor(ptr, EDGE_TOP, 1);
        gtk_layer_set_anchor(ptr, EDGE_LEFT, 1);
        gtk_layer_set_anchor(ptr, EDGE_RIGHT, 1);
        gtk_layer_set_anchor(ptr, EDGE_BOTTOM, 0);

        let side_margin = (monitor_width - width) / 2;
        gtk_layer_set_margin(ptr, EDGE_LEFT, side_margin);
        gtk_layer_set_margin(ptr, EDGE_RIGHT, side_margin);
        gtk_layer_set_margin(ptr, EDGE_TOP, initial_top_margin);
    }
}

pub fn set_top_margin(window: &ApplicationWindow, margin: i32) {
    if !is_supported() {
        return;
    }

    unsafe {
        gtk_layer_set_margin(window_ptr(window), EDGE_TOP, margin);
    }
}

/// Lower the surface below normal windows so external dialogs can appear above.
pub fn lower(window: &ApplicationWindow) {
    if !is_supported() {
        return;
    }

    unsafe {
        gtk_layer_set_layer(window_ptr(window), LAYER_BOTTOM);
    }
}

/// Raise the surface back above normal windows.
pub fn raise(window: &ApplicationWindow) {
    if !is_supported() {
        return;
    }

    unsafe {
        gtk_layer_set_layer(window_ptr(window), LAYER_TOP);
    }
}
