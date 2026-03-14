use crate::platform;
use crate::state::{AppState, HotkeyBinding};
use log::{error, info};
use std::ffi::c_void;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CGEventRef = *mut c_void;

const K_CG_SESSION_EVENT_TAP: u32 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
const K_CG_EVENT_KEY_DOWN: u64 = 10;
const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

const FLAG_CMD: u64 = 0x00100000;
const FLAG_ALT: u64 = 0x00080000;
const FLAG_SHIFT: u64 = 0x00020000;
const FLAG_CTRL: u64 = 0x00040000;

type CGEventTapCallBack = extern "C" fn(
    proxy: *mut c_void,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: CFMachPortRef,
        order: i64,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: *const c_void);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopRun();
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
}

extern "C" {
    static kCFRunLoopCommonModes: *const c_void;
}

struct HotkeyContext {
    state: Arc<AppState>,
    handle: AppHandle,
}


fn js_code_to_mac_keycode(code: &str) -> Option<u16> {
    match code {
        "KeyA" => Some(0x00),
        "KeyS" => Some(0x01),
        "KeyD" => Some(0x02),
        "KeyF" => Some(0x03),
        "KeyH" => Some(0x04),
        "KeyG" => Some(0x05),
        "KeyZ" => Some(0x06),
        "KeyX" => Some(0x07),
        "KeyC" => Some(0x08),
        "KeyV" => Some(0x09),
        "KeyB" => Some(0x0B),
        "KeyQ" => Some(0x0C),
        "KeyW" => Some(0x0D),
        "KeyE" => Some(0x0E),
        "KeyR" => Some(0x0F),
        "KeyY" => Some(0x10),
        "KeyT" => Some(0x11),
        "KeyU" => Some(0x20),
        "KeyI" => Some(0x22),
        "KeyO" => Some(0x1F),
        "KeyP" => Some(0x23),
        "KeyL" => Some(0x25),
        "KeyJ" => Some(0x26),
        "KeyK" => Some(0x28),
        "KeyN" => Some(0x2D),
        "KeyM" => Some(0x2E),
        "Digit1" => Some(0x12),
        "Digit2" => Some(0x13),
        "Digit3" => Some(0x14),
        "Digit4" => Some(0x15),
        "Digit5" => Some(0x17),
        "Digit6" => Some(0x16),
        "Digit7" => Some(0x1A),
        "Digit8" => Some(0x1C),
        "Digit9" => Some(0x19),
        "Digit0" => Some(0x1D),
        "Space" => Some(0x31),
        "Tab" => Some(0x30),
        "F1" => Some(0x7A),
        "F2" => Some(0x78),
        "F3" => Some(0x63),
        "F4" => Some(0x76),
        "F5" => Some(0x60),
        "F6" => Some(0x61),
        "F7" => Some(0x62),
        "F8" => Some(0x64),
        "F9" => Some(0x65),
        "F10" => Some(0x6D),
        "F11" => Some(0x67),
        "F12" => Some(0x6F),
        _ => None,
    }
}

fn matches_binding(keycode: u16, flags: u64, binding: &HotkeyBinding) -> bool {
    let expected = match js_code_to_mac_keycode(&binding.key) {
        Some(k) => k,
        None => return false,
    };
    if keycode != expected {
        return false;
    }
    let has_cmd = flags & FLAG_CMD != 0;
    let has_alt = flags & FLAG_ALT != 0;
    let has_shift = flags & FLAG_SHIFT != 0;
    let has_ctrl = flags & FLAG_CTRL != 0;
    has_cmd == binding.cmd && has_alt == binding.alt && has_shift == binding.shift && has_ctrl == binding.ctrl
}

extern "C" fn hotkey_callback(
    _proxy: *mut c_void,
    _event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if event.is_null() || user_info.is_null() {
        return event;
    }
    let ctx = unsafe { &*(user_info as *const HotkeyContext) };

    let keycode = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u16;
    let flags = unsafe { CGEventGetFlags(event) };

    let hotkeys = ctx.state.get_hotkeys();

    for binding in &hotkeys {
        if matches_binding(keycode, flags, binding) {
            // Sync current index from the actual foreground window before cycling.
            let fg_id = platform::get_foreground_window_id();
            if fg_id != 0 {
                ctx.state.sync_current_from_window_id(fg_id);
            }

            let win = match binding.action.as_str() {
                "next" => ctx.state.cycle_next(),
                "prev" => ctx.state.cycle_prev(),
                "principal" => ctx.state.get_principal(),
                _ => continue,
            };

            if let Some(win) = win {
                let wm = platform::create_window_manager();
                let _ = wm.focus_window(&win);
                let handle = ctx.handle.clone();
                let name = win.character_name.clone();
                std::thread::spawn(move || {
                    let _ = handle.emit("focus-changed", &name);
                });
            }
            break;
        }
    }

    event
}

pub fn start_hotkey_listener(handle: AppHandle, state: Arc<AppState>) {
    let ctx = Box::new(HotkeyContext { state, handle });
    let ctx_addr = Box::into_raw(ctx) as usize;

    std::thread::spawn(move || unsafe {
        let user_info = ctx_addr as *mut c_void;
        let events_mask: u64 = 1 << K_CG_EVENT_KEY_DOWN;

        let tap = CGEventTapCreate(
            K_CG_SESSION_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
            events_mask,
            hotkey_callback,
            user_info,
        );

        if tap.is_null() {
            error!("[Hotkeys] Failed to create CGEventTap — check Accessibility permission");
            return;
        }

        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);

        info!("[Hotkeys] CGEventTap started — listening for global hotkeys");
        CFRunLoopRun();
    });
}
