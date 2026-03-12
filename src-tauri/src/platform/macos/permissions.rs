use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFMutableDictionary;
use core_foundation::string::CFString;
use std::ffi::c_void;

extern "C" {
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

pub fn is_accessibility_enabled() -> bool {
    unsafe {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let mut dict = CFMutableDictionary::new();
        dict.set(key.as_CFType(), CFBoolean::false_value().as_CFType());
        AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as *const c_void)
    }
}

#[allow(dead_code)]
pub fn request_accessibility() {
    unsafe {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let mut dict = CFMutableDictionary::new();
        dict.set(key.as_CFType(), CFBoolean::true_value().as_CFType());
        AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as *const c_void);
    }
}

pub fn is_screen_recording_enabled() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

pub fn request_screen_recording() {
    unsafe {
        CGRequestScreenCaptureAccess();
    }
}
