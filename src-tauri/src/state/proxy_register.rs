// TAKES SERIAL/TCP PORT URL and maps it to a PortManager

use crate::proxy::port_manager::PortManager;
use crate::state::capture::CaptureState;
use dashmap::DashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub struct ProxyRegister {
    pub ports: DashMap<String, Arc<PortManager>>,
    capture: CaptureState,
    app: AppHandle,
}

impl ProxyRegister {
    pub fn new(app: AppHandle, capture: CaptureState) -> Self {
        Self {
            ports: DashMap::new(),
            app,
            capture,
        }
    }

    pub fn app_handle(&self) -> AppHandle {
        self.app.clone()
    }

    pub fn ensure(&self, url: String) {
        let capture_tx = self.capture.inner.command_tx.clone();
        self.ports
            .entry(url.clone())
            .or_insert_with(|| PortManager::new(url, self.app.clone(), capture_tx));
    }

    pub fn prune<F>(&self, keep: F)
    where
        F: Fn(&String) -> bool,
    {
        self.ports.retain(|url, pm| {
            if !url.starts_with("serial://") {
                return true;
            }
            
            if keep(url) {
                true
            } else {
                println!("[Discovery] Pruning disconnected port: {}", url);

                self.app.emit("device-removed", url.clone()).unwrap();

                pm.shutdown();

                false
            }
        });
    }
    pub fn get(&self, url: &String) -> Option<Arc<PortManager>> {
        self.ports.get(url).map(|r| r.value().clone())
    }
}
