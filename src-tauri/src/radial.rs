use crate::platform;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

// Radial geometry — must match RadialSelector.tsx constants
pub const OUTER_R: f64 = 124.0;
pub const INNER_R: f64 = 34.0;
pub const RADIAL_WIN_SIZE: f64 = 350.0;
pub const RADIAL_WIN_CX: f64 = RADIAL_WIN_SIZE / 2.0;

/// Compute the account segment index under the cursor, or None if outside the wheel.
pub fn radial_segment_at(
    cursor_x: f64,
    cursor_y: f64,
    center_x: f64,
    center_y: f64,
    n: usize,
) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let dx = cursor_x - center_x;
    let dy = cursor_y - center_y;
    let dist = (dx * dx + dy * dy).sqrt();
    if !(INNER_R..=OUTER_R).contains(&dist) {
        return None;
    }
    let mut angle = dy.atan2(dx) + std::f64::consts::PI / 2.0;
    if angle < 0.0 {
        angle += 2.0 * std::f64::consts::PI;
    }
    if angle >= 2.0 * std::f64::consts::PI {
        angle -= 2.0 * std::f64::consts::PI;
    }
    Some((angle / (2.0 * std::f64::consts::PI) * n as f64).floor() as usize % n)
}

/// Resolve the selected account name from the cursor's logical position at key/button release.
/// Returns None if the cursor is outside the wheel or no accounts are registered.
pub fn resolve_selection(state: &AppState, logical_x: f64, logical_y: f64) -> Option<String> {
    let keydown = state.get_radial_center()?;
    let accounts = state.get_account_views();
    let n = accounts.len();
    if n == 0 {
        return None;
    }
    let rel_x = RADIAL_WIN_CX + (logical_x - keydown.0);
    let rel_y = RADIAL_WIN_CX + (logical_y - keydown.1);
    let seg = radial_segment_at(rel_x, rel_y, RADIAL_WIN_CX, RADIAL_WIN_CX, n)?;
    Some(accounts[seg].character_name.clone())
}

/// Focus the selected account by name, or fall back to the current window.
/// Emits "focus-changed" on success. Intended to be called from a spawned thread.
pub fn focus_selected_or_current(
    handle: AppHandle,
    state: Arc<AppState>,
    selected: Option<String>,
) {
    let wm = platform::create_window_manager();
    if let Some(name) = selected {
        let windows = wm.list_dofus_windows();
        if let Some(win) = windows
            .iter()
            .find(|w| w.character_name.eq_ignore_ascii_case(&name))
        {
            let _ = wm.focus_window(win);
            state.set_current_by_name(&name);
            let _ = handle.emit("focus-changed", &name);
        }
    } else if let Some(win) = state.get_current_window() {
        let _ = wm.focus_window(&win);
    }
}
