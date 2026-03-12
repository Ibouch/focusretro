use crate::platform::{GameWindow, WindowManager};
use log::info;
use std::mem;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
use windows::Win32::System::Threading::AttachThreadInput;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

pub struct WinWindowManager;

impl WinWindowManager {
    pub fn new() -> Self {
        Self
    }
}

struct EnumData {
    windows: Vec<HWND>,
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let data = &mut *(lparam.0 as *mut EnumData);
    data.windows.push(hwnd);
    TRUE
}

fn enum_all_windows() -> Vec<HWND> {
    let mut data = EnumData { windows: Vec::new() };
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_callback),
            LPARAM(&mut data as *mut EnumData as isize),
        );
    }
    data.windows
}

fn get_window_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len as usize])
}

impl WindowManager for WinWindowManager {
    fn list_dofus_windows(&self) -> Vec<GameWindow> {
        let mut result = Vec::new();
        for hwnd in enum_all_windows() {
            if unsafe { !IsWindowVisible(hwnd).as_bool() } {
                continue;
            }
            let title = get_window_text(hwnd);
            let idx = match title.find(" - Dofus Retro") {
                Some(i) => i,
                None => continue,
            };
            let character_name = title[..idx].trim().to_string();
            if character_name.is_empty() {
                continue;
            }
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            let window_id = hwnd.0 as usize as u64;
            result.push(GameWindow {
                character_name,
                window_id,
                pid,
                title,
            });
        }
        result
    }

    fn focus_window(&self, window: &GameWindow) -> anyhow::Result<()> {
        let hwnd = enum_all_windows()
            .into_iter()
            .find(|&h| get_window_text(h) == window.title)
            .ok_or_else(|| anyhow::anyhow!("Window not found: {}", window.title))?;

        unsafe {
            // Unminimize only if actually minimized — calling SW_RESTORE on a
            // fullscreen window would exit fullscreen and shrink it to windowed.
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }

            // AttachThreadInput trick to bypass Windows focus-stealing prevention
            let fg_hwnd = GetForegroundWindow();
            let fg_tid = GetWindowThreadProcessId(fg_hwnd, None);
            let mut target_pid = 0u32;
            let target_tid = GetWindowThreadProcessId(hwnd, Some(&mut target_pid));

            if fg_tid != target_tid {
                let _ = AttachThreadInput(fg_tid, target_tid, TRUE);
            }
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
            if fg_tid != target_tid {
                let _ = AttachThreadInput(fg_tid, target_tid, BOOL::from(false));
            }
        }

        info!("[WinWindow] Focused window: {}", window.character_name);
        Ok(())
    }

    fn send_enter_key(&self) -> anyhow::Result<()> {
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_RETURN,
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_RETURN,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        let sent = unsafe { SendInput(&inputs, mem::size_of::<INPUT>() as i32) };
        if sent == inputs.len() as u32 {
            info!("[WinWindow] Sent Enter key via SendInput");
            Ok(())
        } else {
            Err(anyhow::anyhow!("SendInput failed (sent {} of 2)", sent))
        }
    }
}
