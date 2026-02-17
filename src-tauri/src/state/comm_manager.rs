use crate::shared::PortState;
use crate::state::port_manager::{PortManager, PortMsg};
use crate::util::get_valid_twinleaf_serial_urls;
use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use twinleaf::tio::proto::identifiers::ColumnKey;
use twinleaf::tio::proto::DeviceRoute;

const LOCAL_TCP_URL: &str = "tcp://localhost";
const DISCOVERY_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Default)]
struct CommModel {
    ports: HashMap<String, Arc<PortManager>>,
    confirmed_selections: HashMap<String, Vec<ColumnKey>>,
    ignored_serial_ports: HashSet<String>,
    selected_port: Option<String>,
    discovery_polling_enabled: bool,
}

pub enum CommMsg {
    Connect {
        port_url: String,
        reply: Sender<Result<Arc<PortManager>, String>>,
    },
    Disconnect {
        port_url: String,
        reply: Sender<Result<(), String>>,
    },
    ConfirmSelection {
        port_url: String,
        children_routes: Vec<DeviceRoute>,
        reply: Sender<Result<(), String>>,
    },
    SetDiscoveryPolling {
        enabled: bool,
    },
    RefreshPort {
        port_url: String,
        reply: Sender<Result<(), String>>,
    },
    GetPort {
        port_url: String,
        reply: Sender<Option<Arc<PortManager>>>,
    },
    ListPorts {
        reply: Sender<Vec<Arc<PortManager>>>,
    },
}

pub struct CommManager {
    tx: Sender<CommMsg>,
}

impl CommManager {
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = unbounded::<CommMsg>();
        Self::spawn_actor(app, rx);
        Self { tx }
    }

    fn spawn_actor(app: AppHandle, rx: Receiver<CommMsg>) {
        thread::Builder::new()
            .name("comm-manager".into())
            .spawn(move || {
                let mut model = CommModel::default();

                loop {
                    match rx.recv_timeout(DISCOVERY_POLL_INTERVAL) {
                        Ok(msg) => Self::update(msg, &mut model, &app),
                        Err(crossbeam::channel::RecvTimeoutError::Timeout) => {}
                        Err(crossbeam::channel::RecvTimeoutError::Disconnected) => break,
                    }

                    if model.discovery_polling_enabled {
                        Self::discovery_tick(&mut model, &app);
                    } else {
                        Self::idle_tick(&mut model, &app);
                    }
                }
            })
            .expect("Failed to spawn comm-manager thread.");
    }

    fn update(msg: CommMsg, model: &mut CommModel, app: &AppHandle) {
        match msg {
            CommMsg::Connect { port_url, reply } => {
                let _ = reply.send(Self::connect(model, app, port_url));
            }
            CommMsg::Disconnect { port_url, reply } => {
                let _ = reply.send(Self::disconnect(model, app, &port_url));
            }
            CommMsg::ConfirmSelection {
                port_url,
                children_routes,
                reply,
            } => {
                let _ = reply.send(Self::confirm_selection(model, &port_url, children_routes));
            }
            CommMsg::SetDiscoveryPolling { enabled } => {
                model.discovery_polling_enabled = enabled;
                if !enabled {
                    Self::idle_tick(model, app);
                }
            }
            CommMsg::RefreshPort { port_url, reply } => {
                let _ = reply.send(Self::refresh_port(model, &port_url));
            }
            CommMsg::GetPort { port_url, reply } => {
                let _ = reply.send(model.ports.get(&port_url).cloned());
            }
            CommMsg::ListPorts { reply } => {
                let ports = model.ports.values().cloned().collect();
                let _ = reply.send(ports);
            }
        }
    }

    fn connect(
        model: &mut CommModel,
        app: &AppHandle,
        port_url: String,
    ) -> Result<Arc<PortManager>, String> {
        if port_url.starts_with("serial://") {
            model.ignored_serial_ports.remove(&port_url);
        }

        model.selected_port = Some(port_url.clone());

        Self::ensure_port(model, app, port_url.clone())
            .ok_or_else(|| format!("Port '{}' is not available.", port_url))
    }

    fn disconnect(model: &mut CommModel, app: &AppHandle, port_url: &str) -> Result<(), String> {
        if port_url.starts_with("serial://") {
            model.ignored_serial_ports.insert(port_url.to_string());
        }

        if let Some(port) = model.ports.remove(port_url) {
            port.shutdown();
        }

        model.confirmed_selections.remove(port_url);
        if model.selected_port.as_deref() == Some(port_url) {
            model.selected_port = None;
        }

        app.emit("device-removed", port_url.to_string())
            .map_err(|e| format!("Failed to emit device-removed for '{}': {}", port_url, e))?;

        Ok(())
    }

    fn confirm_selection(
        model: &mut CommModel,
        port_url: &str,
        children_routes: Vec<DeviceRoute>,
    ) -> Result<(), String> {
        let port_manager = model
            .ports
            .get(port_url)
            .cloned()
            .ok_or_else(|| format!("Port '{}' not found.", port_url))?;

        let state = Self::port_state(&port_manager)
            .ok_or_else(|| format!("Port '{}' state is unavailable.", port_url))?;
        if !matches!(state, PortState::Streaming) {
            return Err(format!(
                "Cannot confirm selection for '{}': expected Streaming, got {:?}.",
                port_url, state
            ));
        }

        let keys = Self::selection_keys_for_routes(&port_manager, port_url, children_routes)?;

        model
            .confirmed_selections
            .insert(port_url.to_string(), keys.clone());
        model.selected_port = Some(port_url.to_string());
        Self::shutdown_serial_except(model, port_url);

        let mut selected_routes: Vec<DeviceRoute> = keys
            .into_iter()
            .map(|key| key.device_route())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        selected_routes.sort();

        if !selected_routes.is_empty() {
            port_manager
                .refresh_rpc_cache_for_routes(&selected_routes)
                .map_err(|e| {
                    format!(
                        "Failed to refresh RPC metadata after selection for '{}': {:?}",
                        port_url, e
                    )
                })?;
        }

        Ok(())
    }

    fn selection_keys_for_routes(
        port_manager: &Arc<PortManager>,
        port_url: &str,
        children_routes: Vec<DeviceRoute>,
    ) -> Result<Vec<ColumnKey>, String> {
        let mut keys = HashSet::new();
        let mut all_selected_routes = children_routes;
        all_selected_routes.push(DeviceRoute::root());

        for route in all_selected_routes {
            let maybe_device = port_manager
                .ask(|reply| PortMsg::GetDevice {
                    route: route.clone(),
                    reply,
                })
                .map_err(|e| {
                    format!(
                        "Failed to query device '{}' on '{}': {}",
                        route, port_url, e
                    )
                })?;

            let Some(device) = maybe_device else {
                continue;
            };

            for stream in &device.streams {
                for column in &stream.columns {
                    keys.insert(ColumnKey::new(
                        route.clone(),
                        stream.meta.stream_id,
                        column.index,
                    ));
                }
            }
        }

        let mut keys: Vec<ColumnKey> = keys.into_iter().collect();
        keys.sort_by(|left, right| {
            left.route
                .cmp(&right.route)
                .then_with(|| left.stream_id.cmp(&right.stream_id))
                .then_with(|| left.column_id.cmp(&right.column_id))
        });
        Ok(keys)
    }

    fn refresh_port(model: &CommModel, port_url: &str) -> Result<(), String> {
        let port = model
            .ports
            .get(port_url)
            .ok_or_else(|| format!("Port '{}' not found.", port_url))?;
        port.rescan();
        Ok(())
    }

    fn is_serial_selected(model: &CommModel, port_url: &str) -> bool {
        match &model.selected_port {
            None => true,
            Some(selected) => {
                if selected.starts_with("serial://") && port_url.starts_with("serial://") {
                    selected == port_url
                } else if selected.starts_with("tcp://") && port_url.starts_with("serial://") {
                    false
                } else {
                    true
                }
            }
        }
    }

    fn port_state(pm: &Arc<PortManager>) -> Option<PortState> {
        pm.ask(|reply| PortMsg::GetState { reply }).ok()
    }

    fn ensure_port(
        model: &mut CommModel,
        app: &AppHandle,
        port_url: String,
    ) -> Option<Arc<PortManager>> {
        if port_url.starts_with("serial://") {
            if model.ignored_serial_ports.contains(&port_url) {
                return None;
            }
            if !Self::is_serial_selected(model, &port_url) {
                return None;
            }
        }

        match model.ports.entry(port_url.clone()) {
            Entry::Occupied(mut occupied) => {
                let pm = occupied.get().clone();
                let state = Self::port_state(&pm);

                if matches!(state, Some(PortState::Disconnected | PortState::Error(_))) {
                    if port_url.starts_with("serial://") {
                        pm.shutdown();
                        let replacement = PortManager::new(port_url.clone(), app.clone());
                        occupied.insert(replacement.clone());
                        Some(replacement)
                    } else {
                        pm.connect();
                        Some(pm)
                    }
                } else {
                    Some(pm)
                }
            }
            Entry::Vacant(vacant) => {
                let pm = PortManager::new(port_url, app.clone());
                vacant.insert(pm.clone());
                Some(pm)
            }
        }
    }

    fn shutdown_serial_except(model: &CommModel, keep_port_url: &str) {
        for (port_url, manager) in &model.ports {
            if port_url.starts_with("serial://") && port_url != keep_port_url {
                manager.shutdown();
            }
        }
    }

    fn prune_serial_ports<F>(model: &mut CommModel, app: &AppHandle, keep: F)
    where
        F: Fn(&str) -> bool,
    {
        let to_remove: Vec<String> = model
            .ports
            .keys()
            .filter(|port_url| port_url.starts_with("serial://") && !keep(port_url))
            .cloned()
            .collect();

        for port_url in to_remove {
            if let Some(manager) = model.ports.remove(&port_url) {
                let _ = app.emit("device-removed", port_url.clone());
                manager.shutdown();
            }

            model.confirmed_selections.remove(&port_url);
            if model.selected_port.as_deref() == Some(port_url.as_str()) {
                model.selected_port = None;
            }
        }
    }

    fn idle_tick(model: &mut CommModel, app: &AppHandle) {
        if let Some(selected_serial) = model
            .selected_port
            .clone()
            .filter(|port_url| port_url.starts_with("serial://"))
        {
            let _ = Self::ensure_port(model, app, selected_serial.clone());
            Self::prune_serial_ports(model, app, |port_url| port_url == selected_serial);
        } else {
            Self::prune_serial_ports(model, app, |_| false);
        }
    }

    fn discovery_tick(model: &mut CommModel, app: &AppHandle) {
        let tcp_url = LOCAL_TCP_URL.to_string();
        let _ = Self::ensure_port(model, app, tcp_url.clone());

        if let Some(tcp_port) = model.ports.get(&tcp_url) {
            let state = Self::port_state(tcp_port);
            if matches!(state, Some(PortState::Error(_) | PortState::Disconnected)) {
                tcp_port.connect();
            }
        }

        let tcp_online = model
            .ports
            .get(&tcp_url)
            .and_then(Self::port_state)
            .as_ref()
            .is_some_and(|state| {
                matches!(
                    state,
                    PortState::Discovery | PortState::Streaming | PortState::Reconnecting
                )
            });

        if tcp_online {
            Self::shutdown_serial_except(model, LOCAL_TCP_URL);
            Self::prune_serial_ports(model, app, |_| false);
            return;
        }

        if model
            .selected_port
            .as_ref()
            .is_some_and(|port_url| port_url.starts_with("tcp://"))
        {
            Self::prune_serial_ports(model, app, |_| false);
            return;
        }

        if let Some(selected_serial) = model
            .selected_port
            .clone()
            .filter(|port_url| port_url.starts_with("serial://"))
        {
            let _ = Self::ensure_port(model, app, selected_serial.clone());
            Self::prune_serial_ports(model, app, |port_url| port_url == selected_serial);
            return;
        }

        let visible_serials: HashSet<String> = get_valid_twinleaf_serial_urls()
            .into_iter()
            .filter(|port_url| !model.ignored_serial_ports.contains(port_url))
            .collect();

        for port_url in &visible_serials {
            let _ = Self::ensure_port(model, app, port_url.clone());
        }

        Self::prune_serial_ports(model, app, |port_url| visible_serials.contains(port_url));
    }

    pub fn dispatch(&self, msg: CommMsg) -> Result<(), String> {
        self.tx
            .send(msg)
            .map_err(|_| "CommManager actor is unavailable.".to_string())
    }

    pub fn ask<R>(&self, build: impl FnOnce(Sender<R>) -> CommMsg) -> Result<R, String>
    where
        R: Send + 'static,
    {
        let (reply_tx, reply_rx) = bounded(1);
        self.tx
            .send(build(reply_tx))
            .map_err(|_| "CommManager actor is unavailable.".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "CommManager actor did not respond.".to_string())
    }
}
