use crate::shared::{
    ColumnMeta, DataColumnId, DeviceMeta, Point, PortState, RpcError, RpcMeta, UiDevice, UiStream,
};
use crate::state::capture::{CaptureCommand, CaptureState, SessionId};
use crate::state::proxy_register::ProxyRegister;
use crate::util::{self, parse_arg_type_and_size, parse_permissions_string};
use crossbeam::channel::{Sender, TrySendError};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, RwLock},
    thread,
    time::{Duration, Instant},
};
use tauri::menu::MenuItemKind;
use tauri::{Emitter, Manager};
use twinleaf::{
    data::{DeviceFullMetadata, Sample},
    device::{DeviceEvent, DeviceTree, RpcClient, TreeEvent, TreeItem},
    tio::{
        proto::{self, DeviceRoute},
        proxy,
    },
};

const LOCAL_TCP_URL: &str = "tcp://localhost";

pub struct DebugCounters {
    pub polls: AtomicUsize,
    pub samples_received: AtomicUsize,
    pub points_inserted: AtomicUsize,
    pub dropped_batches: AtomicUsize,
}

#[derive(Default)]
struct DiscoveryTracker {
    discovered_routes: HashSet<DeviceRoute>,
    ready_routes: HashSet<DeviceRoute>,
}

enum PortReducerEvent {
    ResetToIdle,
    ConnectStarted,
    RouteDiscovered(DeviceRoute),
    RouteReady(DeviceRoute),
    SensorDisconnected,
    SensorReconnected,
    FailedToConnect(String),
    FailedToReconnect(String),
    PollError,
}

#[derive(Debug)]
pub enum PortCommand {
    Shutdown,
    AttemptConnection,
    RescanDevices,
}

pub struct PortManager {
    pub url: String,
    pub state: Mutex<PortState>,
    connection_retries: Mutex<u32>,
    pub proxy: Mutex<Option<Arc<proxy::Interface>>>,
    tree: Mutex<Option<DeviceTree>>,
    rpc_client: Mutex<Option<Arc<Mutex<RpcClient>>>>,
    discovery_tracker: Mutex<DiscoveryTracker>,
    pub devices: RwLock<HashMap<DeviceRoute, Arc<Mutex<UiDevice>>>>,
    pub command_tx: crossbeam::channel::Sender<PortCommand>,
    pub app: tauri::AppHandle,
    pub capture_tx: Sender<CaptureCommand>,
    pub counters: DebugCounters,
}

impl PortManager {
    pub fn new(
        url: String,
        app: tauri::AppHandle,
        capture_tx: Sender<CaptureCommand>,
    ) -> Arc<Self> {
        let (command_tx, command_rx) = crossbeam::channel::unbounded::<PortCommand>();

        let pm = Arc::new(Self {
            url,
            state: Mutex::new(PortState::Idle),
            connection_retries: Mutex::new(0),
            proxy: Mutex::new(None),
            tree: Mutex::new(None),
            rpc_client: Mutex::new(None),
            discovery_tracker: Mutex::new(DiscoveryTracker::default()),
            devices: RwLock::new(HashMap::new()),
            command_tx,
            app,
            capture_tx,
            counters: DebugCounters {
                polls: AtomicUsize::new(0),
                samples_received: AtomicUsize::new(0),
                points_inserted: AtomicUsize::new(0),
                dropped_batches: AtomicUsize::new(0),
            },
        });

        Self::spawn_thread(pm.clone(), command_rx);
        pm
    }

    pub fn shutdown(&self) {
        let _ = self.command_tx.send(PortCommand::Shutdown);
    }

    pub fn connect(&self) {
        let _ = self.command_tx.send(PortCommand::AttemptConnection);
    }

    pub fn rescan(&self) {
        let _ = self.command_tx.send(PortCommand::RescanDevices);
    }

    fn is_local_tcp(&self) -> bool {
        self.url == LOCAL_TCP_URL
    }

    pub fn ensure_rpc_client(&self) -> Result<(), RpcError> {
        {
            let guard = self
                .rpc_client
                .lock()
                .map_err(|_| RpcError::AppLogic("RPC client lock was poisoned.".to_string()))?;
            if guard.is_some() {
                return Ok(());
            }
        }

        let proxy_if = self
            .proxy
            .lock()
            .map_err(|_| RpcError::AppLogic("Proxy lock was poisoned.".to_string()))?
            .clone()
            .ok_or_else(|| RpcError::AppLogic("Proxy interface not available.".to_string()))?;

        let rpc_client = RpcClient::open(&proxy_if, DeviceRoute::root())?;
        let mut guard = self
            .rpc_client
            .lock()
            .map_err(|_| RpcError::AppLogic("RPC client lock was poisoned.".to_string()))?;
        *guard = Some(Arc::new(Mutex::new(rpc_client)));
        Ok(())
    }

    pub fn refresh_rpc_cache_for_routes(&self, routes: &[DeviceRoute]) -> Result<(), RpcError> {
        if routes.is_empty() {
            return Ok(());
        }

        self.ensure_rpc_client()?;

        let rpc_client = self
            .rpc_client
            .lock()
            .map_err(|_| RpcError::AppLogic("RPC client lock was poisoned.".to_string()))?
            .clone()
            .ok_or_else(|| {
                RpcError::AppLogic(
                    "RPC client is not initialized. Select a device first.".to_string(),
                )
            })?;

        let mut seen = HashSet::new();
        for route in routes {
            if !seen.insert(route.clone()) {
                continue;
            }

            let rpcs = {
                let client = rpc_client
                    .lock()
                    .map_err(|_| RpcError::AppLogic("RPC client lock was poisoned.".to_string()))?;
                self.fetch_rpcs(&client, route)
            };

            let maybe_updated = {
                let devices_map = self.devices.read().map_err(|_| {
                    RpcError::AppLogic("Device cache read lock was poisoned.".to_string())
                })?;

                if let Some(device_entry) = devices_map.get(route) {
                    let mut ui_device = device_entry.lock().map_err(|_| {
                        RpcError::AppLogic(format!("Device lock for '{}' was poisoned.", route))
                    })?;

                    ui_device.rpcs = rpcs;
                    Some(ui_device.clone())
                } else {
                    None
                }
            };

            if let Some(ui_device) = maybe_updated {
                let _ = self.app.emit("device-metadata-updated", ui_device);
            }
        }

        Ok(())
    }

    fn clear_runtime_handles(&self) {
        *self.tree.lock().unwrap() = None;
        *self.proxy.lock().unwrap() = None;
        *self.rpc_client.lock().unwrap() = None;
        self.reset_discovery_tracker();
    }

    fn drop_rpc_client(&self) {
        *self.rpc_client.lock().unwrap() = None;
    }

    fn reset_discovery_tracker(&self) {
        *self.discovery_tracker.lock().unwrap() = DiscoveryTracker::default();
    }

    fn mark_route_discovered(&self, route: &DeviceRoute) {
        self.discovery_tracker
            .lock()
            .unwrap()
            .discovered_routes
            .insert(route.clone());
    }

    fn mark_route_ready(&self, route: &DeviceRoute) {
        let mut tracker = self.discovery_tracker.lock().unwrap();
        tracker.discovered_routes.insert(route.clone());
        tracker.ready_routes.insert(route.clone());
    }

    fn all_discovered_routes_ready(&self) -> bool {
        let tracker = self.discovery_tracker.lock().unwrap();
        !tracker.discovered_routes.is_empty()
            && tracker.discovered_routes.is_subset(&tracker.ready_routes)
    }

    fn reduce_port_state(
        current: &PortState,
        event: &PortReducerEvent,
        discovery_ready: bool,
    ) -> Option<PortState> {
        match event {
            PortReducerEvent::ResetToIdle => Some(PortState::Idle),
            PortReducerEvent::ConnectStarted => Some(PortState::Connecting),
            PortReducerEvent::RouteDiscovered(_) => {
                if matches!(
                    current,
                    PortState::Idle
                        | PortState::Connecting
                        | PortState::Disconnected
                        | PortState::Reconnecting
                ) {
                    Some(PortState::Discovery)
                } else {
                    None
                }
            }
            PortReducerEvent::RouteReady(_) => {
                if discovery_ready
                    && matches!(
                        current,
                        PortState::Idle
                            | PortState::Connecting
                            | PortState::Discovery
                            | PortState::Disconnected
                            | PortState::Reconnecting
                    )
                {
                    Some(PortState::Streaming)
                } else if matches!(
                    current,
                    PortState::Idle
                        | PortState::Connecting
                        | PortState::Disconnected
                        | PortState::Reconnecting
                ) {
                    Some(PortState::Discovery)
                } else {
                    None
                }
            }
            PortReducerEvent::SensorDisconnected | PortReducerEvent::PollError => {
                Some(PortState::Disconnected)
            }
            PortReducerEvent::SensorReconnected => Some(PortState::Reconnecting),
            PortReducerEvent::FailedToConnect(msg) | PortReducerEvent::FailedToReconnect(msg) => {
                Some(PortState::Error(msg.clone()))
            }
        }
    }

    fn dispatch_port_event(&self, event: PortReducerEvent) {
        match &event {
            PortReducerEvent::ResetToIdle | PortReducerEvent::ConnectStarted => {
                self.reset_discovery_tracker();
            }
            PortReducerEvent::RouteDiscovered(route) => self.mark_route_discovered(route),
            PortReducerEvent::RouteReady(route) => self.mark_route_ready(route),
            PortReducerEvent::SensorDisconnected
            | PortReducerEvent::PollError
            | PortReducerEvent::FailedToConnect(_)
            | PortReducerEvent::FailedToReconnect(_) => self.reset_discovery_tracker(),
            PortReducerEvent::SensorReconnected => self.reset_discovery_tracker(),
        }

        let current = self.state.lock().unwrap().clone();
        let discovery_ready = self.all_discovered_routes_ready();
        let Some(next) = Self::reduce_port_state(&current, &event, discovery_ready) else {
            return;
        };
        if next == current {
            return;
        }

        let should_emit_reconnected = matches!(&event, PortReducerEvent::SensorReconnected);
        self.set_state(next.clone());

        match next {
            PortState::Streaming => {
                self.set_all_cached_device_states(PortState::Streaming, true);
            }
            PortState::Disconnected => {
                self.drop_rpc_client();
                self.set_all_cached_device_states(PortState::Disconnected, true);
            }
            PortState::Error(_) => {
                self.drop_rpc_client();
            }
            state @ (PortState::Idle
            | PortState::Connecting
            | PortState::Discovery
            | PortState::Reconnecting) => {
                self.set_all_cached_device_states(state, true);
                if should_emit_reconnected {
                    self.emit_reconnected_selection_if_any();
                }
            }
        }
    }

    fn handle_command(self: &Arc<Self>, command: PortCommand) -> bool {
        match command {
            PortCommand::Shutdown => {
                if !self.is_local_tcp() {
                    println!("[{}] Shutdown command received.", self.url);
                }
                false
            }
            PortCommand::AttemptConnection => {
                if !self.is_local_tcp() {
                    println!("[{}] Manual connection attempt triggered.", self.url);
                }
                let current_state = self.state.lock().unwrap().clone();
                if matches!(current_state, PortState::Error(_) | PortState::Disconnected) {
                    self.clear_runtime_handles();
                    self.dispatch_port_event(PortReducerEvent::ResetToIdle);
                }
                true
            }
            PortCommand::RescanDevices => {
                println!("[{}] Manual device rescan triggered.", self.url);
                self.rebuild_tree_for_rescan();
                true
            }
        }
    }

    fn spawn_thread(self_: Arc<Self>, command_rx: crossbeam::channel::Receiver<PortCommand>) {
        thread::Builder::new()
            .name(format!("port-{}", self_.url))
            .spawn(move || {
                let mut last_debug_print = Instant::now();
                let mut shutdown_requested = false;

                'lifecycle: loop {
                    loop {
                        match command_rx.try_recv() {
                            Ok(command) => {
                                let is_shutdown = matches!(command, PortCommand::Shutdown);
                                if !self_.handle_command(command) {
                                    shutdown_requested = shutdown_requested || is_shutdown;
                                    break 'lifecycle;
                                }
                            }
                            Err(crossbeam::channel::TryRecvError::Empty) => break,
                            Err(crossbeam::channel::TryRecvError::Disconnected) => {
                                eprintln!("[{}] Command channel broke. Shutting down.", self_.url);
                                break 'lifecycle;
                            }
                        }
                    }

                    let current_state = self_.state.lock().unwrap().clone();
                    let has_tree = self_.tree.lock().unwrap().is_some();

                    if !has_tree && current_state == PortState::Idle {
                        self_.attempt_open_tree();
                        continue;
                    }

                    if has_tree {
                        self_.poll_tree_data();

                        if last_debug_print.elapsed() > Duration::from_secs(30) {
                            let polls = self_.counters.polls.swap(0, Ordering::Relaxed);
                            let points = self_.counters.points_inserted.swap(0, Ordering::Relaxed);
                            let batch = self_.counters.dropped_batches.swap(0, Ordering::Relaxed);

                            let state = self_.state.lock().unwrap().clone();

                            if !matches!(state, PortState::Error(_)) {
                                println!(
                                    "[{}] Heartbeat (30s): State={:?}, Polls={}, PointsIns={}, DroppedBatches={}",
                                    self_.url, state, polls, points, batch
                                );
                            }

                            last_debug_print = Instant::now();
                        }
                        continue;
                    }

                    match command_rx.recv_timeout(Duration::from_millis(250)) {
                        Ok(command) => {
                            let is_shutdown = matches!(command, PortCommand::Shutdown);
                            if !self_.handle_command(command) {
                                shutdown_requested = shutdown_requested || is_shutdown;
                                break 'lifecycle;
                            }
                        }
                        Err(crossbeam::channel::RecvTimeoutError::Timeout) => {}
                        Err(crossbeam::channel::RecvTimeoutError::Disconnected) => {
                            eprintln!("[{}] Command channel broke. Shutting down.", self_.url);
                            break 'lifecycle;
                        }
                    }
                }

                if !self_.is_local_tcp() {
                    println!("[{}] Port manager thread cleaning up and shutting down.", self_.url);
                }
                self_.clear_runtime_handles();
                self_.devices.write().unwrap().clear();
                if !shutdown_requested && !matches!(*self_.state.lock().unwrap(), PortState::Error(_)) {
                    self_.dispatch_port_event(PortReducerEvent::PollError);
                }
            })
            .expect("Failed to spawn PortManager thread.");
    }

    fn attempt_open_tree(&self) {
        self.dispatch_port_event(PortReducerEvent::ConnectStarted);

        if !self.is_local_tcp() {
            println!(
                "[{}] Attempting to establish proxy/tree connection...",
                self.url
            );
        }
        let proxy_if = Arc::new(proxy::Interface::new_proxy(
            &self.url,
            Some(Duration::from_secs(30)),
            None,
        ));

        match DeviceTree::open(&proxy_if, DeviceRoute::root()) {
            Ok(tree) => {
                *self.connection_retries.lock().unwrap() = 0;
                *self.proxy.lock().unwrap() = Some(proxy_if);
                *self.tree.lock().unwrap() = Some(tree);
                self.drop_rpc_client();
            }
            Err(e) => {
                // Serial ownership conflicts are usually persistent while a mux is running.
                // Don't loop aggressively there; TCP can keep retrying.
                let max_retries: u32 = if self.url.starts_with("serial://") {
                    1
                } else {
                    10
                };
                let retry_delay = if self.url.starts_with("serial://") {
                    Duration::from_millis(250)
                } else {
                    Duration::from_secs(2)
                };
                let mut retries = self.connection_retries.lock().unwrap();
                *retries += 1;

                if *retries >= max_retries {
                    let err_msg = format!(
                        "Failed to connect after {} attempts. Giving up: {:?}",
                        max_retries, e
                    );
                    if !self.is_local_tcp() {
                        eprintln!("[{}] {}", self.url, err_msg);
                    }
                    self.dispatch_port_event(PortReducerEvent::FailedToConnect(err_msg));
                    *self.proxy.lock().unwrap() = None;
                    *self.tree.lock().unwrap() = None;
                    *retries = 0;
                } else {
                    if !self.is_local_tcp() {
                        println!(
                            "[{}] Failed to connect tree (attempt {}/{}). Retrying in {:?}...",
                            self.url, *retries, max_retries, retry_delay
                        );
                    }
                    self.dispatch_port_event(PortReducerEvent::ResetToIdle);
                    *self.proxy.lock().unwrap() = None;
                    *self.tree.lock().unwrap() = None;
                    thread::sleep(retry_delay);
                }
            }
        }
    }

    fn rebuild_tree_for_rescan(&self) {
        self.drop_rpc_client();
        self.dispatch_port_event(PortReducerEvent::ConnectStarted);
        self.devices.write().unwrap().clear();

        let proxy_if = match self.proxy.lock().unwrap().clone() {
            Some(proxy_if) => proxy_if,
            None => {
                *self.tree.lock().unwrap() = None;
                self.dispatch_port_event(PortReducerEvent::ResetToIdle);
                return;
            }
        };

        match DeviceTree::open(&proxy_if, DeviceRoute::root()) {
            Ok(tree) => {
                *self.tree.lock().unwrap() = Some(tree);
            }
            Err(e) => {
                let err_msg = format!("Rescan failed to open tree: {:?}", e);
                eprintln!("[{}] {}", self.url, err_msg);
                *self.tree.lock().unwrap() = None;
                self.dispatch_port_event(PortReducerEvent::FailedToConnect(err_msg));
            }
        }
    }

    fn emit_reconnected_selection_if_any(&self) {
        let cached_keys = {
            let registry = self.app.state::<Arc<ProxyRegister>>();
            registry
                .active_selections
                .get(&self.url)
                .map(|keys| keys.value().clone())
        };

        if let Some(keys_for_port) = cached_keys {
            println!(
                "[{}] Re-applying cached selection of {} columns.",
                self.url,
                keys_for_port.len()
            );

            let capture_state = self.app.state::<CaptureState>();
            let command = crate::state::capture::CaptureCommand::SetActiveColumns {
                port_url: self.url.clone(),
                keys_for_port: keys_for_port.clone(),
            };

            if let Err(e) = capture_state.inner.command_tx.send(command) {
                eprintln!(
                    "[{}] Failed to send SetActiveColumns command: {}",
                    self.url, e
                );
            }

            let mut child_routes: Vec<String> = keys_for_port
                .iter()
                .map(|k| k.device_route.to_string())
                .filter(|r| !r.is_empty())
                .collect();
            child_routes.sort();
            child_routes.dedup();

            let payload = serde_json::json!({
                "portUrl": self.url,
                "childrenRoutes": child_routes,
            });

            if let Err(e) = self.app.emit("device-reconnected", payload) {
                eprintln!("[{}] Failed to emit device-reconnected: {}", self.url, e);
            }
        }
    }

    fn set_all_cached_device_states(&self, state: PortState, emit_updates: bool) {
        let devices_map = self.devices.read().unwrap();
        for device_entry in devices_map.values() {
            let mut ui_device = device_entry.lock().unwrap();
            ui_device.state = state.clone();
            if emit_updates {
                let _ = self.app.emit("device-metadata-updated", ui_device.clone());
            }
        }
    }

    fn emit_cached_devices_snapshot(&self) {
        let mut devices = Vec::new();
        let devices_map = self.devices.read().unwrap();
        for device_entry in devices_map.values() {
            devices.push(device_entry.lock().unwrap().clone());
        }

        devices.sort_by(|a, b| a.route.cmp(&b.route));
        if let Err(e) = self.app.emit("port-devices-discovered", devices) {
            eprintln!(
                "[{}] Failed to emit port-devices-discovered: {}",
                self.url, e
            );
        }
    }

    fn handle_tree_event(self: &Arc<Self>, event: TreeEvent, discovered_batch: &mut Vec<UiDevice>) {
        match event {
            TreeEvent::RouteDiscovered(route) => {
                println!("[{}] Route discovered: {}", self.url, route);
                self.dispatch_port_event(PortReducerEvent::RouteDiscovered(route));
            }
            TreeEvent::Device { route, event } => {
                self.handle_device_event(route, event, discovered_batch);
            }
        }
    }

    fn handle_device_event(
        self: &Arc<Self>,
        route: DeviceRoute,
        event: DeviceEvent,
        discovered_batch: &mut Vec<UiDevice>,
    ) {
        match event {
            DeviceEvent::Status(status) => match status {
                proto::ProxyStatus::SensorReconnected => {
                    *self.connection_retries.lock().unwrap() = 0;
                    self.dispatch_port_event(PortReducerEvent::SensorReconnected);
                }
                proto::ProxyStatus::SensorDisconnected => {
                    if !self.is_local_tcp() {
                        println!(
                            "[{}] Sensor disconnected. Marking port as disconnected.",
                            self.url
                        );
                    }
                    self.dispatch_port_event(PortReducerEvent::SensorDisconnected);
                }
                proto::ProxyStatus::FailedToReconnect => {
                    let err_msg = "Proxy failed to reconnect after timeout.".to_string();
                    if !self.is_local_tcp() {
                        eprintln!("[{}] {}", self.url, err_msg);
                    }
                    self.dispatch_port_event(PortReducerEvent::FailedToReconnect(err_msg));
                }
                proto::ProxyStatus::FailedToConnect => {
                    let err_msg = "Proxy failed to connect.".to_string();
                    if !self.is_local_tcp() {
                        eprintln!("[{}] {}", self.url, err_msg);
                    }
                    self.dispatch_port_event(PortReducerEvent::FailedToConnect(err_msg));
                }
                proto::ProxyStatus::Unknown(code) => {
                    eprintln!("[{}] Unknown proxy status received: {}", self.url, code);
                }
            },
            DeviceEvent::RpcInvalidated(method) => {
                self.invalidate_rpc_cache(&route, method);
            }
            DeviceEvent::Heartbeat { .. } => {}
            DeviceEvent::MetadataReady(full_metadata) => {
                self.dispatch_port_event(PortReducerEvent::RouteReady(route.clone()));
                let (ui_device, is_new) = self.apply_full_metadata(&route, full_metadata);
                if is_new {
                    discovered_batch.push(ui_device.clone());
                } else if let Err(e) = self.app.emit("device-metadata-updated", ui_device) {
                    eprintln!(
                        "[{}] Failed to emit device-metadata-updated: {}",
                        self.url, e
                    );
                }
            }
        }
    }

    fn invalidate_rpc_cache(&self, route: &DeviceRoute, method: proto::RpcMethod) {
        let maybe_updated = {
            let devices_map = match self.devices.read() {
                Ok(guard) => guard,
                Err(_) => return,
            };

            let Some(device_entry) = devices_map.get(route) else {
                return;
            };

            let mut ui_device = match device_entry.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };

            let mut changed = false;

            match method {
                proto::RpcMethod::Name(name) => {
                    if let Some(rpc) = ui_device.rpcs.iter_mut().find(|r| r.name == name) {
                        rpc.value = None;
                        changed = true;
                    }
                }
                proto::RpcMethod::Id(_) => {
                    for rpc in &mut ui_device.rpcs {
                        if rpc.value.is_some() {
                            rpc.value = None;
                            changed = true;
                        }
                    }
                }
            }

            if changed {
                Some(ui_device.clone())
            } else {
                None
            }
        };

        if let Some(ui_device) = maybe_updated {
            let _ = self.app.emit("device-metadata-updated", ui_device);
        }
    }

    fn apply_full_metadata(
        &self,
        route: &DeviceRoute,
        full_metadata: DeviceFullMetadata,
    ) -> (UiDevice, bool) {
        let streams = self.ui_streams_from_metadata(&full_metadata);
        self.update_capture_state_with_stream_metadata(route, &streams);

        let state = self.state.lock().unwrap().clone();

        let mut devices = self.devices.write().unwrap();
        if let Some(entry) = devices.get(route) {
            let mut ui_device = entry.lock().unwrap();
            ui_device.state = state;
            ui_device.meta = DeviceMeta::from((*full_metadata.device).clone());
            ui_device.streams = streams;
            (ui_device.clone(), false)
        } else {
            let ui_device = UiDevice {
                url: self.url.clone(),
                route: route.to_string(),
                state,
                meta: DeviceMeta::from((*full_metadata.device).clone()),
                streams,
                rpcs: Vec::new(),
            };
            devices.insert(route.clone(), Arc::new(Mutex::new(ui_device.clone())));
            (ui_device, true)
        }
    }

    fn ui_streams_from_metadata(&self, full_metadata: &DeviceFullMetadata) -> Vec<UiStream> {
        let mut sorted_streams: Vec<_> = full_metadata.streams.values().collect();
        sorted_streams.sort_by_key(|s| s.stream.stream_id);

        sorted_streams
            .into_iter()
            .map(|s| {
                let mut ui_columns: Vec<ColumnMeta> = s
                    .columns
                    .iter()
                    .map(|c| ColumnMeta::from((**c).clone()))
                    .collect();
                ui_columns.sort_by_key(|c| c.index);

                let segment_data = (*s.segment).clone();
                let decimation = if segment_data.decimation > 0 {
                    segment_data.decimation as f64
                } else {
                    1.0
                };
                let effective_sampling_rate = segment_data.sampling_rate as f64 / decimation;

                UiStream {
                    meta: (*s.stream).clone().into(),
                    segment: Some(segment_data.into()),
                    columns: ui_columns,
                    effective_sampling_rate,
                }
            })
            .collect()
    }

    fn ui_stream_from_sample(sample: &Sample) -> UiStream {
        let mut ui_columns: Vec<ColumnMeta> = sample
            .columns
            .iter()
            .map(|c| ColumnMeta::from((*c.desc).clone()))
            .collect();
        ui_columns.sort_by_key(|c| c.index);

        let segment_data = (*sample.segment).clone();
        let decimation = if segment_data.decimation > 0 {
            segment_data.decimation as f64
        } else {
            1.0
        };
        let effective_sampling_rate = segment_data.sampling_rate as f64 / decimation;

        UiStream {
            meta: (*sample.stream).clone().into(),
            segment: Some(segment_data.into()),
            columns: ui_columns,
            effective_sampling_rate,
        }
    }

    fn ensure_device_from_sample(&self, route: &DeviceRoute, sample: &Sample) -> Option<UiDevice> {
        let stream = Self::ui_stream_from_sample(sample);
        let state = self.state.lock().unwrap().clone();

        let mut devices = self.devices.write().unwrap();
        if devices.contains_key(route) {
            return None;
        }

        let ui_device = UiDevice {
            url: self.url.clone(),
            route: route.to_string(),
            state,
            meta: DeviceMeta::from((*sample.device).clone()),
            streams: vec![stream.clone()],
            rpcs: Vec::new(),
        };

        self.update_capture_state_with_stream_metadata(route, &[stream]);

        devices.insert(route.clone(), Arc::new(Mutex::new(ui_device.clone())));
        Some(ui_device)
    }

    fn fetch_rpcs(&self, rpc_client: &RpcClient, route: &DeviceRoute) -> Vec<RpcMeta> {
        println!("[{}]   -> Fetching RPCs for route '{}'...", self.url, route);
        let mut rpc_metas = Vec::new();

        let rpc_count: u16 = match rpc_client.get(route, "rpc.listinfo") {
            Ok(count) => count,
            Err(e) => {
                eprintln!(
                    "[{}]   -> Failed to query rpc.listinfo for '{}': {:?}",
                    self.url, route, e
                );
                return rpc_metas;
            }
        };

        for rpc_id in 0..rpc_count {
            if let Ok((meta_bits, name)) =
                rpc_client.rpc::<u16, (u16, String)>(route, "rpc.listinfo", rpc_id)
            {
                let (arg_type, size) = parse_arg_type_and_size(meta_bits);

                let mut meta = RpcMeta {
                    name: name.clone(),
                    size,
                    permissions: parse_permissions_string(meta_bits),
                    arg_type: arg_type.clone(),
                    readable: (meta_bits & 0x0100) != 0,
                    writable: (meta_bits & 0x0200) != 0,
                    persistent: (meta_bits & 0x0400) != 0,
                    unknown: meta_bits == 0,
                    value: None,
                };

                if meta.readable {
                    let read_res: Result<Value, proxy::RpcError> = match arg_type.as_str() {
                        "u8" => rpc_client.get::<u8>(route, &name).map(|v| json!(v)),
                        "u16" => rpc_client.get::<u16>(route, &name).map(|v| json!(v)),
                        "u32" => rpc_client.get::<u32>(route, &name).map(|v| json!(v)),
                        "u64" => rpc_client.get::<u64>(route, &name).map(|v| json!(v)),
                        "i8" => rpc_client.get::<i8>(route, &name).map(|v| json!(v)),
                        "i16" => rpc_client.get::<i16>(route, &name).map(|v| json!(v)),
                        "i32" => rpc_client.get::<i32>(route, &name).map(|v| json!(v)),
                        "i64" => rpc_client.get::<i64>(route, &name).map(|v| json!(v)),
                        "f32" => rpc_client.get::<f32>(route, &name).map(|v| json!(v)),
                        "f64" => rpc_client.get::<f64>(route, &name).map(|v| json!(v)),
                        "string" => rpc_client.get::<String>(route, &name).map(|v| json!(v)),
                        _ if arg_type.starts_with("string<") => {
                            rpc_client.get::<String>(route, &name).map(|v| json!(v))
                        }
                        _ => match rpc_client.raw_rpc(route, &name, &[]) {
                            Ok(reply_bytes) => {
                                Ok(util::bytes_to_json_value(&reply_bytes, &arg_type)
                                    .unwrap_or(Value::Null))
                            }
                            Err(e) => Err(e),
                        },
                    };

                    match read_res {
                        Ok(v) => meta.value = Some(v),
                        Err(proxy::RpcError::TypeError) => {
                            meta.value = Some(Value::Null);
                        }
                        Err(e) => {
                            eprintln!(
                                "[{}]   -> Read '{}' on '{}' failed: {:?}",
                                self.url, name, route, e
                            );
                        }
                    }
                }

                rpc_metas.push(meta);
            }
        }

        rpc_metas
    }

    fn poll_tree_data(self: &Arc<Self>) {
        struct BatchedEntry {
            points: Vec<Point>,
            sample_numbers: Vec<u32>,
        }

        let mut discovered_batch: Vec<UiDevice> = Vec::new();
        let mut batched: HashMap<(DataColumnId, SessionId), BatchedEntry> = HashMap::new();
        let poll_instant = Instant::now();
        let (item_result, pending_events) = {
            let mut tree_guard = self.tree.lock().unwrap();
            let Some(tree) = tree_guard.as_mut() else {
                return;
            };
            self.counters.polls.fetch_add(1, Ordering::Relaxed);
            let item = tree.next_item();
            let events = tree.drain_events();
            (item, events)
        };

        match item_result {
            Ok(TreeItem::Event(event)) => {
                self.handle_tree_event(event, &mut discovered_batch);
            }
            Ok(TreeItem::Sample(sample, route)) => {
                self.counters
                    .samples_received
                    .fetch_add(1, Ordering::Relaxed);
                self.dispatch_port_event(PortReducerEvent::RouteDiscovered(route.clone()));

                if let Some(new_ui_device) = self.ensure_device_from_sample(&route, &sample) {
                    discovered_batch.push(new_ui_device);
                }

                let sid = sample.device.session_id;
                let sample_number = sample.n;

                for col in &sample.columns {
                    let Some(val) = col.value.try_as_f64() else {
                        continue;
                    };

                    let key = DataColumnId {
                        port_url: self.url.clone(),
                        device_route: route.clone(),
                        stream_id: sample.stream.stream_id,
                        column_index: col.desc.index,
                    };

                    let entry = batched.entry((key, sid)).or_insert_with(|| BatchedEntry {
                        points: Vec::new(),
                        sample_numbers: Vec::new(),
                    });

                    entry.points.push(Point {
                        x: sample.timestamp_end(),
                        y: val,
                    });
                    entry.sample_numbers.push(sample_number);
                }
            }
            Err(e) => {
                if !self.is_local_tcp() {
                    eprintln!("[{}] Tree polling error: {:?}", self.url, e);
                }
                self.clear_runtime_handles();
                self.dispatch_port_event(PortReducerEvent::PollError);
                return;
            }
        }

        for event in pending_events {
            self.handle_tree_event(event, &mut discovered_batch);
        }

        if !discovered_batch.is_empty() {
            self.emit_cached_devices_snapshot();
        }

        for ((key, sid), entry) in batched {
            let len = entry.points.len();
            match self.capture_tx.try_send(CaptureCommand::InsertBatch {
                key,
                points: entry.points,
                sample_numbers: entry.sample_numbers,
                session_id: sid,
                instant: poll_instant,
            }) {
                Ok(()) => {
                    self.counters
                        .points_inserted
                        .fetch_add(len, Ordering::Relaxed);
                }
                Err(TrySendError::Full(_)) => {
                    self.counters
                        .dropped_batches
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(TrySendError::Disconnected(_)) => {
                    return;
                }
            }
        }
    }

    fn set_state(&self, new_state: PortState) {
        *self.state.lock().unwrap() = new_state.clone();
        self.app
            .emit("port-state-changed", (self.url.clone(), new_state.clone()))
            .unwrap();
        if !self.is_local_tcp() || matches!(new_state, PortState::Streaming | PortState::Error(_)) {
            println!("[{}] Emit new port state {:?}", self.url, new_state);
        }

        if let Some(window) = self.app.get_webview_window("main") {
            if let Some(menu) = window.menu() {
                let is_connected = new_state == PortState::Streaming;

                if let Some(file_menu) = util::find_submenu_by_text(&menu, "File") {
                    if let Some(MenuItemKind::MenuItem(item)) = file_menu.get("save_recording") {
                        item.set_enabled(is_connected).unwrap();
                    }
                }

                if let Some(edit_menu) = util::find_submenu_by_text(&menu, "Edit") {
                    if let Some(MenuItemKind::MenuItem(item)) = edit_menu.get("clear_session") {
                        item.set_enabled(!is_connected).unwrap();
                    }
                }

                if let Some(device_menu) = util::find_submenu_by_text(&menu, "Device") {
                    if let Some(MenuItemKind::MenuItem(item)) = device_menu.get("toggle_logging") {
                        item.set_enabled(is_connected).unwrap();
                    }
                    if let Some(MenuItemKind::MenuItem(item)) = device_menu.get("rpc_settings") {
                        item.set_enabled(is_connected).unwrap();
                    }
                    if let Some(MenuItemKind::MenuItem(item)) = device_menu.get("connect_device") {
                        let text = if is_connected {
                            "Change Device..."
                        } else {
                            "Connect Device..."
                        };
                        item.set_text(text).unwrap();
                    }
                }
            }
        }
    }

    pub async fn execute_rpc(
        &self,
        device_route_str: &str,
        name: &str,
        args: Option<Value>,
    ) -> Result<Value, RpcError> {
        let route = DeviceRoute::from_str(device_route_str).map_err(|_| {
            RpcError::AppLogic(format!(
                "Invalid device route string: '{}'",
                device_route_str
            ))
        })?;

        let rpc_meta = {
            let devices_map = self.devices.read().map_err(|_| {
                RpcError::AppLogic("Device cache read lock was poisoned.".to_string())
            })?;

            let device_entry = devices_map.get(&route).ok_or_else(|| {
                RpcError::AppLogic(format!("Device '{}' not found in cache.", route))
            })?;

            let ui_device = device_entry.lock().map_err(|_| {
                RpcError::AppLogic(format!("Device lock for '{}' was poisoned.", route))
            })?;

            ui_device
                .rpcs
                .iter()
                .find(|r| r.name == name)
                .cloned()
                .ok_or_else(|| RpcError::AppLogic(format!("RPC '{}' not found.", name)))?
        };

        let rpc_client = self
            .rpc_client
            .lock()
            .map_err(|_| RpcError::AppLogic("RPC client lock was poisoned.".to_string()))?
            .clone()
            .ok_or_else(|| {
                RpcError::AppLogic(
                    "RPC client is not initialized. Select a device before executing RPCs."
                        .to_string(),
                )
            })?;

        let rpc_task_result = {
            let route_clone = route.clone();
            let name_clone = name.to_string();
            let args_clone = args.clone();
            let arg_type = rpc_meta.arg_type.clone();

            tauri::async_runtime::spawn_blocking(move || -> Result<Value, RpcError> {
                let client = rpc_client
                    .lock()
                    .map_err(|_| RpcError::AppLogic("RPC client lock was poisoned.".to_string()))?;

                let arg_bytes = util::json_to_bytes(args_clone, &arg_type)?;
                let reply_bytes = client.raw_rpc(&route_clone, &name_clone, &arg_bytes)?;
                util::bytes_to_json_value(&reply_bytes, &arg_type)
                    .ok_or_else(|| RpcError::AppLogic("Failed to parse RPC reply".to_string()))
            })
            .await
            .map_err(|e| RpcError::AppLogic(format!("RPC task panicked: {}", e)))?
        };

        let rpc_result = rpc_task_result?;

        let new_value_to_cache = if rpc_meta.writable && args.is_some() {
            args
        } else if rpc_meta.readable {
            Some(rpc_result.clone())
        } else {
            None
        };

        if let Some(new_val) = new_value_to_cache {
            let devices_map = self.devices.read().map_err(|_| {
                RpcError::AppLogic("Device cache read lock was poisoned.".to_string())
            })?;

            if let Some(device_entry) = devices_map.get(&route) {
                let mut ui_device = device_entry.lock().map_err(|_| {
                    RpcError::AppLogic(format!("Device lock for '{}' was poisoned.", route))
                })?;

                if let Some(cached_rpc) = ui_device.rpcs.iter_mut().find(|r| r.name == name) {
                    cached_rpc.value = Some(new_val);
                    if let Err(e) = self.app.emit("device-metadata-updated", ui_device.clone()) {
                        eprintln!(
                            "[{}] Failed to emit device-metadata-updated event: {}",
                            self.url, e
                        );
                    }
                }
            }
        }

        Ok(rpc_result)
    }

    fn update_capture_state_with_stream_metadata(&self, route: &DeviceRoute, streams: &[UiStream]) {
        for stream in streams {
            let stream_key = DataColumnId {
                port_url: self.url.clone(),
                device_route: route.clone(),
                stream_id: stream.meta.stream_id,
                column_index: 0,
            };

            let command = CaptureCommand::UpdateSampleRate {
                key: stream_key,
                rate: stream.effective_sampling_rate,
            };

            if let Err(e) = self.capture_tx.send(command) {
                eprintln!(
                    "[{}] Failed to send sample rate for '{}' to capture thread: {}",
                    self.url, route, e
                );
            }
        }
    }
}
