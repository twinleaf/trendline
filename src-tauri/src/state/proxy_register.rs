// TAKES SERIAL/TCP PORT URL and maps it to a PortManager

use crate::proxy::port_manager::PortManager;
use crate::shared::{DataColumnId, PortState};
use crate::state::capture::CaptureState;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter};

pub struct ProxyRegister {
    pub ports: DashMap<String, Arc<PortManager>>,
    pub active_selections: DashMap<String, Vec<DataColumnId>>,
    selected_port: RwLock<Option<String>>,
    capture: CaptureState,
    app: AppHandle,
}

impl ProxyRegister {
    pub fn new(app: AppHandle, capture: CaptureState) -> Self {
        Self {
            ports: DashMap::new(),
            active_selections: DashMap::new(),
            selected_port: RwLock::new(None),
            app,
            capture,
        }
    }

    pub fn set_selected_port(&self, url: Option<String>) {
        *self.selected_port.write().unwrap() = url;
    }

    pub fn selected(&self) -> Option<String> {
        self.selected_port.read().unwrap().clone()
    }

    fn is_selected(&self, url: &str) -> bool {
        match &*self.selected_port.read().unwrap() {
            None => true,
            Some(sel) => sel == url,
        }
    }

    pub fn shutdown_all_except(&self, keep_url: &str) {
        for entry in self.ports.iter() {
            let url = entry.key();
            if url.starts_with("serial://") && url != keep_url {
                entry.value().shutdown();
            }
        }
    }

    pub fn app_handle(&self) -> AppHandle {
        self.app.clone()
    }

    pub fn ensure(&self, url: String) {
        if url.starts_with("serial://") && !self.is_selected(&url) {
            if let Some(pm_ref) = self.ports.get(&url) {
                let pm = pm_ref.value().clone();
                let state = pm.state.lock().unwrap().clone();
                if !matches!(state, PortState::Disconnected) {
                    println!("[Registry] Shutting down unselected port '{}'.", url);
                    pm.shutdown();
                }
            }
            return;
        }
        let capture_tx = self.capture.inner.command_tx.clone();
        match self.ports.entry(url.clone()) {
            Entry::Occupied(mut occ) => {
                let pm = occ.get().clone();
                let state = pm.state.lock().unwrap().clone();

                if matches!(state, PortState::Disconnected) {
                    println!(
                        "[Registry] Recreating PortManager for '{}' (was Disconnected).",
                        url
                    );
                    pm.shutdown();
                    let new_pm = PortManager::new(url.clone(), self.app.clone(), capture_tx);
                    occ.insert(new_pm);
                }
            }
            Entry::Vacant(v) => {
                v.insert(PortManager::new(url, self.app.clone(), capture_tx));
            }
        }
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
