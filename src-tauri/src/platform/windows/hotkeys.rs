use crate::platform;
use crate::state::{AppState, HotkeyBinding};
use log::{error, info};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    MSLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
};

struct HotkeyContext {
    state: Arc<AppState>,
    handle: AppHandle,
}

thread_local! {
    static HOTKEY_CTX: std::cell::RefCell<Option<HotkeyContext>> =
        std::cell::RefCell::new(None);
}

fn js_code_to_vk(code: &str) -> Option<u16> {
    match code {
        // Letters A–Z → virtual keycodes 0x41–0x5A
        "KeyA" => Some(0x41),
        "KeyB" => Some(0x42),
        "KeyC" => Some(0x43),
        "KeyD" => Some(0x44),
        "KeyE" => Some(0x45),
        "KeyF" => Some(0x46),
        "KeyG" => Some(0x47),
        "KeyH" => Some(0x48),
        "KeyI" => Some(0x49),
        "KeyJ" => Some(0x4A),
        "KeyK" => Some(0x4B),
        "KeyL" => Some(0x4C),
        "KeyM" => Some(0x4D),
        "KeyN" => Some(0x4E),
        "KeyO" => Some(0x4F),
        "KeyP" => Some(0x50),
        "KeyQ" => Some(0x51),
        "KeyR" => Some(0x52),
        "KeyS" => Some(0x53),
        "KeyT" => Some(0x54),
        "KeyU" => Some(0x55),
        "KeyV" => Some(0x56),
        "KeyW" => Some(0x57),
        "KeyX" => Some(0x58),
        "KeyY" => Some(0x59),
        "KeyZ" => Some(0x5A),
        // Digits 0–9 → 0x30–0x39
        "Digit0" => Some(0x30),
        "Digit1" => Some(0x31),
        "Digit2" => Some(0x32),
        "Digit3" => Some(0x33),
        "Digit4" => Some(0x34),
        "Digit5" => Some(0x35),
        "Digit6" => Some(0x36),
        "Digit7" => Some(0x37),
        "Digit8" => Some(0x38),
        "Digit9" => Some(0x39),
        // Function keys F1–F12 → 0x70–0x7B
        "F1" => Some(0x70),
        "F2" => Some(0x71),
        "F3" => Some(0x72),
        "F4" => Some(0x73),
        "F5" => Some(0x74),
        "F6" => Some(0x75),
        "F7" => Some(0x76),
        "F8" => Some(0x77),
        "F9" => Some(0x78),
        "F10" => Some(0x79),
        "F11" => Some(0x7A),
        "F12" => Some(0x7B),
        // Special keys
        "Space" => Some(0x20),
        "Tab" => Some(0x09),
        _ => None,
    }
}

fn read_modifiers() -> (bool, bool, bool, bool) {
    unsafe {
        let shift = (GetKeyState(VK_SHIFT.0 as i32) as u16) & 0x8000 != 0;
        let ctrl = (GetKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0;
        let alt = (GetKeyState(VK_MENU.0 as i32) as u16) & 0x8000 != 0;
        let cmd = ((GetKeyState(VK_LWIN.0 as i32) as u16)
            | (GetKeyState(VK_RWIN.0 as i32) as u16))
            & 0x8000
            != 0;
        (shift, ctrl, alt, cmd)
    }
}

fn fire_action(action: &str, c: &HotkeyContext) {
    match action {
        "next" => {
            if let Some(win) = c.state.cycle_next() {
                let wm = platform::create_window_manager();
                let _ = wm.focus_window(&win);
                let _ = c.handle.emit("turn-switched", &win.character_name);
            }
        }
        "prev" => {
            if let Some(win) = c.state.cycle_prev() {
                let wm = platform::create_window_manager();
                let _ = wm.focus_window(&win);
                let _ = c.handle.emit("turn-switched", &win.character_name);
            }
        }
        "principal" => {
            if let Some(win) = c.state.get_principal() {
                let wm = platform::create_window_manager();
                let _ = wm.focus_window(&win);
                let _ = c.handle.emit("turn-switched", &win.character_name);
            }
        }
        _ => {}
    }
}

fn matches_keyboard_binding(
    vk: u16,
    shift: bool,
    ctrl: bool,
    alt: bool,
    cmd: bool,
    binding: &HotkeyBinding,
) -> bool {
    let expected = match js_code_to_vk(&binding.key) {
        Some(k) => k,
        None => return false,
    };
    vk == expected
        && shift == binding.shift
        && ctrl == binding.ctrl
        && alt == binding.alt
        && cmd == binding.cmd
}

fn matches_mouse_binding(
    button: &str,
    shift: bool,
    ctrl: bool,
    alt: bool,
    cmd: bool,
    binding: &HotkeyBinding,
) -> bool {
    binding.key == button
        && shift == binding.shift
        && ctrl == binding.ctrl
        && alt == binding.alt
        && cmd == binding.cmd
}

unsafe extern "system" fn hotkey_callback(
    ncode: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if ncode < 0 {
        return CallNextHookEx(HHOOK::default(), ncode, wparam, lparam);
    }

    // Only act on key-down events (also WM_SYSKEYDOWN for Alt+key combos)
    let msg_id = wparam.0 as u32;
    if msg_id != WM_KEYDOWN && msg_id != WM_SYSKEYDOWN {
        return CallNextHookEx(HHOOK::default(), ncode, wparam, lparam);
    }

    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let vk = kb.vkCode as u16;
    let (shift, ctrl, alt, cmd) = read_modifiers();

    HOTKEY_CTX.with(|ctx| {
        if let Some(ref c) = *ctx.borrow() {
            let hotkeys = c.state.get_hotkeys();
            for binding in &hotkeys {
                if matches_keyboard_binding(vk, shift, ctrl, alt, cmd, binding) {
                    fire_action(&binding.action.clone(), c);
                }
            }
        }
    });

    CallNextHookEx(HHOOK::default(), ncode, wparam, lparam)
}

unsafe extern "system" fn mouse_callback(
    ncode: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if ncode < 0 {
        return CallNextHookEx(HHOOK::default(), ncode, wparam, lparam);
    }

    if wparam.0 as u32 != WM_XBUTTONDOWN {
        return CallNextHookEx(HHOOK::default(), ncode, wparam, lparam);
    }

    let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
    let xbutton = (ms.mouseData >> 16) as u16;
    let button = match xbutton {
        1 => "Mouse4",
        2 => "Mouse5",
        _ => return CallNextHookEx(HHOOK::default(), ncode, wparam, lparam),
    };

    let (shift, ctrl, alt, cmd) = read_modifiers();

    HOTKEY_CTX.with(|ctx| {
        if let Some(ref c) = *ctx.borrow() {
            let hotkeys = c.state.get_hotkeys();
            for binding in &hotkeys {
                if matches_mouse_binding(button, shift, ctrl, alt, cmd, binding) {
                    fire_action(&binding.action.clone(), c);
                }
            }
        }
    });

    CallNextHookEx(HHOOK::default(), ncode, wparam, lparam)
}

pub fn start_hotkey_listener(handle: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        HOTKEY_CTX.with(|ctx| {
            *ctx.borrow_mut() = Some(HotkeyContext {
                state: state.clone(),
                handle: handle.clone(),
            });
        });

        let kb_hook = match unsafe {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(hotkey_callback), None, 0)
        } {
            Ok(h) => h,
            Err(e) => {
                error!("[Hotkeys] WH_KEYBOARD_LL failed: {:?}", e);
                return;
            }
        };

        let mouse_hook = match unsafe {
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_callback), None, 0)
        } {
            Ok(h) => h,
            Err(e) => {
                error!("[Hotkeys] WH_MOUSE_LL failed: {:?}", e);
                unsafe { let _ = UnhookWindowsHookEx(kb_hook); }
                return;
            }
        };

        info!("[Hotkeys] WH_KEYBOARD_LL + WH_MOUSE_LL hooks installed");

        unsafe {
            let mut msg = MSG::default();
            loop {
                let result = GetMessageW(&mut msg, None, 0, 0);
                if result.0 <= 0 {
                    break;
                }
            }
            let _ = UnhookWindowsHookEx(kb_hook);
            let _ = UnhookWindowsHookEx(mouse_hook);
        }

        info!("[Hotkeys] Hooks uninstalled");
    });
}
