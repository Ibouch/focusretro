use crate::platform::{GameWindow, WindowManager};
use core_foundation::base::TCFType;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::display::CGWindowListCopyWindowInfo;
use core_graphics::window::{
    kCGNullWindowID, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
};
use log::{debug, info};
use std::ffi::c_void;
use std::ptr;

type AXUIElementRef = *mut c_void;
type AXError = i32;

const K_AX_SUCCESS: AXError = 0;

type CGEventRef = *mut c_void;
type CGEventSourceRef = *mut c_void;

extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: *mut *mut c_void,
    ) -> AXError;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: *const c_void) -> AXError;
    fn CFRelease(cf: *const c_void);
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventPost(tap: u32, event: CGEventRef);
}

const DOFUS_TITLE_PATTERN: &str = " - Dofus Retro";

pub struct MacWindowManager;

impl MacWindowManager {
    pub fn new() -> Self {
        Self
    }

    fn parse_character_name(title: &str) -> Option<String> {
        if let Some(idx) = title.find(DOFUS_TITLE_PATTERN) {
            let name = title[..idx].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        None
    }
}

impl WindowManager for MacWindowManager {
    fn list_dofus_windows(&self) -> Vec<GameWindow> {
        let mut result = Vec::new();

        let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
        let window_list = unsafe { CGWindowListCopyWindowInfo(options, kCGNullWindowID) };

        if window_list.is_null() {
            return result;
        }

        let window_list_ptr = window_list as *const c_void;
        let count = unsafe { CFArrayGetCount(window_list_ptr) };

        for i in 0..count {
            let dict = unsafe { CFArrayGetValueAtIndex(window_list_ptr, i) };
            if dict.is_null() {
                continue;
            }

            let title = get_dict_string(dict, "kCGWindowName");
            let owner = get_dict_string(dict, "kCGWindowOwnerName");
            let window_id = get_dict_i64(dict, "kCGWindowNumber").unwrap_or(0) as u64;
            let pid = get_dict_i64(dict, "kCGWindowOwnerPID").unwrap_or(0) as u32;

            if let Some(title) = title {
                if let Some(char_name) = Self::parse_character_name(&title) {
                    debug!(
                        "Found Dofus window: {} (owner={}, pid={}, wid={})",
                        title,
                        owner.as_deref().unwrap_or("?"),
                        pid,
                        window_id
                    );
                    result.push(GameWindow {
                        character_name: char_name,
                        window_id,
                        pid,
                        title,
                    });
                }
            }
        }

        unsafe { CFRelease(window_list_ptr) };
        result
    }

    fn focus_window(&self, window: &GameWindow) -> anyhow::Result<()> {
        let pid = window.pid as i32;
        activate_app(pid)?;
        raise_window_ax(pid, &window.title)?;
        Ok(())
    }

    fn send_enter_key(&self) -> anyhow::Result<()> {
        const K_VK_RETURN: u16 = 0x24;
        const K_CG_HID_EVENT_TAP: u32 = 0;

        unsafe {
            let key_down = CGEventCreateKeyboardEvent(ptr::null_mut(), K_VK_RETURN, true);
            let key_up = CGEventCreateKeyboardEvent(ptr::null_mut(), K_VK_RETURN, false);
            if key_down.is_null() || key_up.is_null() {
                return Err(anyhow::anyhow!("Failed to create CGEvent for Enter key"));
            }
            CGEventPost(K_CG_HID_EVENT_TAP, key_down);
            CGEventPost(K_CG_HID_EVENT_TAP, key_up);
            CFRelease(key_down as *const c_void);
            CFRelease(key_up as *const c_void);
        }
        info!("[WindowManager] Sent Enter keypress");
        Ok(())
    }
}

fn activate_app(pid: i32) -> anyhow::Result<()> {
    std::process::Command::new("osascript")
        .args([
            "-e",
            &format!(
                "tell application \"System Events\" to set frontmost of (first process whose unix id is {}) to true",
                pid
            ),
        ])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to activate app: {}", e))?;
    Ok(())
}

fn raise_window_ax(pid: i32, target_title: &str) -> anyhow::Result<()> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return Err(anyhow::anyhow!("Failed to create AXUIElement for pid {}", pid));
        }

        let windows_attr = CFString::new("AXWindows");
        let mut windows_value: *mut c_void = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            app_element,
            windows_attr.as_concrete_TypeRef() as *const c_void,
            &mut windows_value,
        );

        if err != K_AX_SUCCESS || windows_value.is_null() {
            CFRelease(app_element as *const c_void);
            return Err(anyhow::anyhow!("Failed to get AXWindows (error {})", err));
        }

        let count = CFArrayGetCount(windows_value as *const c_void);

        for i in 0..count {
            let win_element =
                CFArrayGetValueAtIndex(windows_value as *const c_void, i) as AXUIElementRef;
            if win_element.is_null() {
                continue;
            }

            let title_attr = CFString::new("AXTitle");
            let mut title_value: *mut c_void = std::ptr::null_mut();
            let title_err = AXUIElementCopyAttributeValue(
                win_element,
                title_attr.as_concrete_TypeRef() as *const c_void,
                &mut title_value,
            );

            if title_err == K_AX_SUCCESS && !title_value.is_null() {
                let cf_title = CFString::wrap_under_get_rule(title_value as *const _);
                let win_title = cf_title.to_string();

                if win_title == target_title {
                    let raise_action = CFString::new("AXRaise");
                    AXUIElementPerformAction(
                        win_element,
                        raise_action.as_concrete_TypeRef() as *const c_void,
                    );
                    break;
                }
            }
        }

        CFRelease(windows_value as *const c_void);
        CFRelease(app_element as *const c_void);
    }

    Ok(())
}

fn get_dict_string(dict: *const c_void, key: &str) -> Option<String> {
    unsafe {
        let cf_key = CFString::new(key);
        let value = CFDictionaryGetValue(dict, cf_key.as_concrete_TypeRef() as *const c_void);
        if value.is_null() {
            return None;
        }
        let cf_str = CFString::wrap_under_get_rule(value as *const _);
        Some(cf_str.to_string())
    }
}

fn get_dict_i64(dict: *const c_void, key: &str) -> Option<i64> {
    unsafe {
        let cf_key = CFString::new(key);
        let value = CFDictionaryGetValue(dict, cf_key.as_concrete_TypeRef() as *const c_void);
        if value.is_null() {
            return None;
        }
        let cf_num = CFNumber::wrap_under_get_rule(value as *const _);
        cf_num.to_i64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_character_name() {
        assert_eq!(
            MacWindowManager::parse_character_name("Craette - Dofus Retro v1.40.0"),
            Some("Craette".to_string())
        );
        assert_eq!(
            MacWindowManager::parse_character_name("My-Char_123 - Dofus Retro v1.39.0"),
            Some("My-Char_123".to_string())
        );
        assert_eq!(
            MacWindowManager::parse_character_name("Some Random Window"),
            None
        );
    }
}
