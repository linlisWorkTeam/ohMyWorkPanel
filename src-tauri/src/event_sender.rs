use core::fmt;

/// Generic event sender that works with both Tauri AppHandle and WebSocket broadcast.
#[derive(Clone)]
pub enum EventSender {
    #[cfg(feature = "gui")]
    Tauri(tauri::AppHandle),
    Web(tokio::sync::broadcast::Sender<String>),
}

impl fmt::Debug for EventSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "gui")]
            EventSender::Tauri(_) => write!(f, "EventSender::Tauri"),
            EventSender::Web(_) => write!(f, "EventSender::Web"),
        }
    }
}
