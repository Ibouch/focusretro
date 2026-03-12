use crate::core::{accounts, parser};
use crate::platform;
use crate::state::{AppState, StoredMessage};
use log::{debug, error, info};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub fn setup(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let state = app.state::<Arc<AppState>>().inner().clone();

    refresh_accounts(&handle, &state);

    start_notification_listener(handle.clone(), state.clone());

    let poll_handle = handle.clone();
    let poll_state = state.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        refresh_accounts(&poll_handle, &poll_state);
    });

    info!("FocusRetro autoswitch initialized");
    Ok(())
}

fn refresh_accounts(handle: &AppHandle, state: &Arc<AppState>) {
    let windows = accounts::detect_accounts();
    state.update_accounts(windows);
    let views = state.get_account_views();
    let _ = handle.emit("accounts-updated", &views);
    crate::update_tray_display(handle, state);
}

fn focus_character_with_fallback(character_name: &str, auto_accept: bool) {
    let name = character_name.to_string();
    let wm = platform::create_window_manager();
    let windows = wm.list_dofus_windows();
    info!(
        "[Autoswitch] Found {} Dofus windows: {:?}",
        windows.len(),
        windows
            .iter()
            .map(|w| &w.character_name)
            .collect::<Vec<_>>()
    );

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        info!("[Autoswitch] Running fallback direct focus for {}", name);
        let wm_fallback = platform::create_window_manager();
        let windows = wm_fallback.list_dofus_windows();
        if let Some(win) = windows
            .iter()
            .find(|w| w.character_name.eq_ignore_ascii_case(&name))
        {
            if let Err(e) = wm_fallback.focus_window(win) {
                error!("[Autoswitch] Fallback focus failed: {}", e);
            } else {
                info!("[Autoswitch] Fallback focus succeeded for {}", name);
            }
        }

        if auto_accept {
            std::thread::sleep(std::time::Duration::from_millis(300));
            info!("[Autoswitch] Auto-accept: sending Enter for {}", name);
            let wm_enter = platform::create_window_manager();
            if let Err(e) = wm_enter.send_enter_key() {
                error!("[Autoswitch] Auto-accept Enter failed: {}", e);
            }
        }
    });
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn start_notification_listener(handle: AppHandle, state: Arc<AppState>) {
    let listener = platform::create_notification_listener();
    let callback_handle = handle.clone();
    let callback_state = state.clone();

    std::thread::spawn(move || {
        let result = listener.start(Box::new(move |segments| {
            debug!("[Autoswitch] Notification segments: {:?}", segments);

            let event = match parser::parse_game_event(&segments) {
                Some(e) => e,
                None => {
                    info!("[Autoswitch] No game event matched");
                    return false;
                }
            };

            match event {
                parser::GameEvent::Turn(turn) => {
                    if !callback_state.is_autoswitch_enabled() {
                        info!("[Autoswitch] autoswitch disabled, ignoring turn");
                        return false;
                    }
                    info!("[Autoswitch] Turn detected for: {}", turn.character_name);
                    let _ = callback_handle.emit("turn-switched", &turn.character_name);
                    focus_character_with_fallback(&turn.character_name, false);
                    true
                }
                parser::GameEvent::GroupInvite(invite) => {
                    if !callback_state.is_group_invite_enabled() {
                        info!("[Autoswitch] group invite disabled, ignoring");
                        return false;
                    }
                    if !callback_state.has_account(&invite.inviter_name) {
                        info!(
                            "[Autoswitch] group invite from unknown '{}', ignoring",
                            invite.inviter_name
                        );
                        return false;
                    }
                    info!(
                        "[Autoswitch] Group invite: {} invited by {}",
                        invite.receiver_name, invite.inviter_name
                    );
                    let _ = callback_handle.emit("group-invite", &invite.receiver_name);
                    let accept = callback_state.is_auto_accept_enabled();
                    focus_character_with_fallback(&invite.receiver_name, accept);
                    true
                }
                parser::GameEvent::Trade(trade) => {
                    if !callback_state.is_trade_enabled() {
                        info!("[Autoswitch] trade disabled, ignoring");
                        return false;
                    }
                    if !callback_state.has_account(&trade.requester_name) {
                        info!(
                            "[Autoswitch] trade from unknown '{}', ignoring",
                            trade.requester_name
                        );
                        return false;
                    }
                    info!(
                        "[Autoswitch] Trade request: {} from {}",
                        trade.receiver_name, trade.requester_name
                    );
                    let _ = callback_handle.emit("trade-request", &trade.receiver_name);
                    let accept = callback_state.is_auto_accept_enabled();
                    focus_character_with_fallback(&trade.receiver_name, accept);
                    true
                }
                parser::GameEvent::PrivateMessage(pm) => {
                    if !callback_state.is_pm_enabled() {
                        info!("[Autoswitch] PM disabled, ignoring");
                        return false;
                    }
                    info!(
                        "[Autoswitch] PM from {} to {}: {}",
                        pm.sender_name, pm.receiver_name, pm.message
                    );
                    let stored = StoredMessage {
                        receiver: pm.receiver_name.clone(),
                        sender: pm.sender_name.clone(),
                        message: pm.message.clone(),
                        timestamp: now_epoch_secs(),
                    };
                    callback_state.add_message(stored.clone());
                    let _ = callback_handle.emit("new-pm", &stored);
                    false
                }
            }
        }));

        if let Err(e) = result {
            error!("[Autoswitch] Notification listener failed: {}", e);
        }
    });
}
