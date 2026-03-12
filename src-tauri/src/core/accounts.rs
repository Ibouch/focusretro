use crate::platform::{self, GameWindow};

pub fn detect_accounts() -> Vec<GameWindow> {
    let wm = platform::create_window_manager();
    wm.list_dofus_windows()
}
