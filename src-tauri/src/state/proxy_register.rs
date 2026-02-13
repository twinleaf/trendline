// TAKES SERIAL/TCP PORT URL and maps it to a PortManager

use crate::proxy::port_manager::PortManager;
use crate::shared::{DataColumnId, PortState};
use crate::state::capture::CaptureState;
use dashmap::mapref::entry::Entry;
use dashmap::{DashMap, DashSet};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter};

pub struct ProxyRegister {
    pub ports: DashMap<String, Arc<PortManager>>,
    pub active_selections: DashMap<String, Vec<DataColumnId>>,
    ignored_serial_ports: DashSet<String>,
    selected_port: RwLock<Option<String>>,
    capture: CaptureState,
    app: AppHandle,
}

impl ProxyRegister {
    pub fn new(app: AppHandle, capture: CaptureState) -> Self {
        Self {
            ports: DashMap::new(),
            active_selections: DashMap::new(),
            ignored_serial_ports: DashSet::new(),
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

    pub fn ignore_serial(&self, url: &str) {
        if url.starts_with("serial://") {
            self.ignored_serial_ports.insert(url.to_string());
        }
    }

    pub fn allow_serial(&self, url: &str) {
        if url.starts_with("serial://") {
            self.ignored_serial_ports.remove(url);
        }
    }

    pub fn is_serial_ignored(&self, url: &str) -> bool {
        url.starts_with("serial://") && self.ignored_serial_ports.contains(url)
    }

    fn is_selected(&self, url: &str) -> bool {
        match &*self.selected_port.read().unwrap() {
            None => true,
            Some(sel) => {
                if sel.starts_with("serial://") && url.starts_with("serial://") {
                    sel == url
                } else if sel.starts_with("tcp://") && url.starts_with("serial://") {
                    false
                } else {
                    true
                }
            }
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
        if url.starts_with("serial://") {
            if self.is_serial_ignored(&url) {
                return;
            }
            if !self.is_selected(&url) {
                return;
            }
        }
        let capture_tx = self.capture.inner.command_tx.clone();
        match self.ports.entry(url.clone()) {
            Entry::Occupied(mut occ) => {
                let pm = occ.get().clone();
                let state = pm.state.lock().unwrap().clone();

                if matches!(state, PortState::Disconnected) {
                    if url.starts_with("serial://") {
                        println!(
                            "[Registry] Recreating PortManager for '{}' (was Disconnected).",
                            url
                        );
                        pm.shutdown();
                        let new_pm = PortManager::new(url.clone(), self.app.clone(), capture_tx);
                        occ.insert(new_pm);
                    } else {
                        pm.connect();
                    }
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
