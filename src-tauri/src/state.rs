use std::sync::{Arc, Mutex};

use crate::settings::Settings;

#[derive(Debug, Clone)]
pub struct BridgeInfo {
    pub port: u16,
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeState {
    pub settings: Settings,
    pub remote_url: Option<String>,
    pub secure_context_applied: bool,
    #[cfg(target_os = "linux")]
    pub secure_proxy: Option<Arc<crate::loopback_proxy::LoopbackProxy>>,
    pub overlay: bool,
    pub remote_label: Option<String>,
    pub generation: u64,
}

impl RuntimeState {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            remote_url: None,
            secure_context_applied: false,
            #[cfg(target_os = "linux")]
            secure_proxy: None,
            overlay: false,
            remote_label: None,
            generation: 0,
        }
    }
}

pub struct AppState {
    pub runtime: Mutex<RuntimeState>,
    pub bridge: BridgeInfo,
    pub capture_lock: Arc<Mutex<()>>,
    /// Latest status report from the remote page for the VRChat chatbox panel.
    pub remote_status: Mutex<Option<crate::vrchat::StatusSnapshot>>,
}
