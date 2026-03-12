use crate::platform::NotificationListener;
use log::{error, info, warn};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use windows::Foundation::Collections::IVectorView;
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{AdaptiveNotificationText, NotificationKinds, UserNotification};
use windows::core::HSTRING;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};

/// Wraps a `Send`-only `Fn` so it can be shared across WinRT threads.
/// Safety: `Fn` uses `&self`, so concurrent calls are safe given captured data is `Send`.
struct SyncCallback(Box<dyn Fn(Vec<String>) -> bool + Send + 'static>);
unsafe impl Sync for SyncCallback {}

pub struct WinNotificationListener {
    thread_id: Arc<Mutex<Option<u32>>>,
}

impl WinNotificationListener {
    pub fn new() -> Self {
        Self {
            thread_id: Arc::new(Mutex::new(None)),
        }
    }
}

/// Process a single notification by ID and invoke the callback if it has toast text.
fn process_notification(
    listener: &UserNotificationListener,
    notif_id: u32,
    cb: &SyncCallback,
) {
    let notif = match listener.GetNotification(notif_id) {
        Ok(n) => n,
        Err(e) => {
            error!("[WinNotif] GetNotification({}) failed: {:?}", notif_id, e);
            return;
        }
    };

    let notification = match notif.Notification() {
        Ok(n) => n,
        Err(e) => {
            error!("[WinNotif] notif.Notification() failed: {:?}", e);
            return;
        }
    };

    // UserNotification::Notification() returns Windows.UI.Notifications.Notification,
    // NOT ToastNotification. Use the Visual → GetBinding → GetTextElements path.
    let visual = match notification.Visual() {
        Ok(v) => v,
        Err(e) => {
            error!("[WinNotif] notification.Visual() failed: {:?}", e);
            return;
        }
    };

    let binding = match visual.GetBinding(&HSTRING::from("ToastGeneric")) {
        Ok(b) => b,
        Err(e) => {
            error!("[WinNotif] GetBinding(ToastGeneric) failed: {:?}", e);
            return;
        }
    };

    let elements: IVectorView<AdaptiveNotificationText> = match binding.GetTextElements() {
        Ok(e) => e,
        Err(e) => {
            error!("[WinNotif] GetTextElements() failed: {:?}", e);
            return;
        }
    };

    let count = elements.Size().unwrap_or(0);
    info!("[WinNotif] Notification {} has {} text element(s)", notif_id, count);

    let mut texts: Vec<String> = Vec::new();
    for i in 0..count {
        match elements.GetAt(i) {
            Ok(elem) => match elem.Text() {
                Ok(t) => {
                    let s = t.to_string();
                    info!("[WinNotif]   text[{}]: {:?}", i, s);
                    if !s.is_empty() {
                        texts.push(s);
                    }
                }
                Err(e) => error!("[WinNotif] elem.Text() failed at {}: {:?}", i, e),
            },
            Err(e) => error!("[WinNotif] GetAt({}) failed: {:?}", i, e),
        }
    }

    if !texts.is_empty() {
        let title = texts[0].clone();
        let body = texts.get(1).cloned().unwrap_or_default();
        let combined = if body.is_empty() {
            title.clone()
        } else {
            format!("{} {}", title, body)
        };
        let segments = vec!["Dofus Retro".to_string(), combined, title, body];
        (cb.0)(segments);
    } else {
        warn!("[WinNotif] Notification {} had no text, skipping callback", notif_id);
    }
}

impl NotificationListener for WinNotificationListener {
    fn start(
        &self,
        on_notification: Box<dyn Fn(Vec<String>) -> bool + Send + 'static>,
    ) -> anyhow::Result<()> {
        let thread_id_store = Arc::clone(&self.thread_id);
        let callback = Arc::new(SyncCallback(on_notification));

        std::thread::spawn(move || {
            unsafe {
                let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                if hr.is_err() {
                    error!("[WinNotif] CoInitializeEx failed: {:?}", hr);
                    return;
                }
            }

            let tid = unsafe { GetCurrentThreadId() };
            {
                let mut guard = thread_id_store.lock().unwrap();
                *guard = Some(tid);
            }

            let listener = match UserNotificationListener::Current() {
                Ok(l) => l,
                Err(e) => {
                    error!("[WinNotif] Failed to get UserNotificationListener: {:?}", e);
                    return;
                }
            };

            let status = match listener.RequestAccessAsync().and_then(|op| op.get()) {
                Ok(s) => s,
                Err(e) => {
                    error!("[WinNotif] Failed to request notification access: {:?}", e);
                    return;
                }
            };

            if status != UserNotificationListenerAccessStatus::Allowed {
                warn!(
                    "[WinNotif] Notification access not granted (status: {:?})",
                    status
                );
                return;
            }

            info!("[WinNotif] Notification access granted, starting poll loop (100ms)");

            let mut seen_ids: HashSet<u32> = HashSet::new();

            // Seed seen_ids with notifications already present so we don't replay old ones.
            if let Ok(op) = listener.GetNotificationsAsync(NotificationKinds::Toast) {
                if let Ok(existing) = op.get() {
                    let existing: IVectorView<UserNotification> = existing;
                    let count = existing.Size().unwrap_or(0);
                    for i in 0..count {
                        if let Ok(n) = existing.GetAt(i) {
                            if let Ok(id) = n.Id() {
                                seen_ids.insert(id);
                            }
                        }
                    }
                }
            }

            info!("[WinNotif] Seeded {} existing notification IDs", seen_ids.len());

            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));

                let notifications: IVectorView<UserNotification> = match listener
                    .GetNotificationsAsync(NotificationKinds::Toast)
                    .and_then(|op| op.get())
                {
                    Ok(n) => n,
                    Err(e) => {
                        error!("[WinNotif] GetNotificationsAsync failed: {:?}", e);
                        break;
                    }
                };

                let count = notifications.Size().unwrap_or(0);
                for i in 0..count {
                    let notif: UserNotification = match notifications.GetAt(i) {
                        Ok(n) => n,
                        Err(_) => continue,
                    };
                    let id = match notif.Id() {
                        Ok(id) => id,
                        Err(_) => continue,
                    };
                    if seen_ids.insert(id) {
                        info!("[WinNotif] Detected new notification ID: {}", id);
                        process_notification(&listener, id, &callback);
                    }
                }

                // Stop if WM_QUIT was posted to this thread.
                // Peek without blocking: use PeekMessageW to drain the quit message.
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::{
                        PM_REMOVE, PeekMessageW, MSG,
                    };
                    let mut msg = MSG::default();
                    if PeekMessageW(&mut msg, None, WM_QUIT, WM_QUIT, PM_REMOVE).as_bool() {
                        break;
                    }
                }
            }

            info!("[WinNotif] Notification listener thread exiting");
        });

        Ok(())
    }

    fn stop(&self) {
        let tid = {
            let guard = self.thread_id.lock().unwrap();
            *guard
        };
        if let Some(thread_id) = tid {
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            info!(
                "[WinNotif] Posted WM_QUIT to notification thread {}",
                thread_id
            );
        }
    }
}
