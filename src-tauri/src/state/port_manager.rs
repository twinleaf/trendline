use crate::shared::{PortState, RpcError, RpcMeta as UiRpcMeta, UiDevice, UiStream};
use crate::state::data_manager::{DataManager, DataMsg};
use crate::state::rpc_manager::{RpcManager, RpcMsg};
use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use twinleaf::data::{AlignedWindow, DeviceFullMetadata, ReadError};
use twinleaf::device::{DeviceEvent, DeviceTree, RpcMeta as TwinleafRpcMeta, TreeEvent, TreeItem};
use twinleaf::tio::proto::identifiers::ColumnKey;
use twinleaf::tio::proto::{DeviceRoute, ProxyStatus, RpcMethod};
use twinleaf::tio::proxy;

const DATA_BUFFER_CAPACITY: usize = 500_000;
const PROXY_RECONNECT_TIMEOUT: Duration = Duration::from_secs(3);

pub enum PortMsg {
    Connect,
    Shutdown,
    Rescan,
    GetState {
        reply: Sender<PortState>,
    },
    ExecuteRpc {
        route: DeviceRoute,
        name: String,
        args: Value,
        reply: Sender<Result<Value, RpcError>>,
    },
    GetDevices {
        reply: Sender<Vec<UiDevice>>,
    },
    GetDevice {
        route: DeviceRoute,
        reply: Sender<Option<UiDevice>>,
    },
    ReadWindow {
        columns: Vec<ColumnKey>,
        n: usize,
        reply: Sender<Result<AlignedWindow, ReadError>>,
    },
    ReadTimeRange {
        columns: Vec<ColumnKey>,
        start_time: f64,
        end_time: f64,
        reply: Sender<Result<AlignedWindow, ReadError>>,
    },
    RefreshRpcCacheForRoutes {
        routes: Vec<DeviceRoute>,
        reply: Sender<Result<(), RpcError>>,
    },
    CreateSnapshot {
        id: String,
        columns: Vec<ColumnKey>,
        start_time: f64,
        end_time: f64,
        reply: Sender<Result<(), ReadError>>,
    },
    GetSnapshot {
        id: String,
        reply: Sender<Option<AlignedWindow>>,
    },
    ClearSnapshot {
        id: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeToken(u64);

enum PumpEvent {
    TreeEvent {
        token: RuntimeToken,
        event: TreeEvent,
    },
    Fault {
        token: RuntimeToken,
        error: String,
    },
    Exited {
        token: RuntimeToken,
    },
}

#[derive(Default)]
struct DiscoveryTracker {
    discovered: HashSet<DeviceRoute>,
    ready: HashSet<DeviceRoute>,
}

impl DiscoveryTracker {
    fn clear(&mut self) {
        self.discovered.clear();
        self.ready.clear();
    }

    fn mark_discovered(&mut self, route: DeviceRoute) {
        self.discovered.insert(route);
    }

    fn mark_ready(&mut self, route: DeviceRoute) {
        self.ready.insert(route);
    }

    fn all_ready(&self) -> bool {
        !self.discovered.is_empty() && self.discovered.is_subset(&self.ready)
    }
}

struct PortRuntime {
    token: RuntimeToken,
    proxy: Arc<proxy::Interface>,
    pump_join: Option<std::thread::JoinHandle<()>>,
}

struct PortModel {
    state: PortState,
    discovery_tracker: DiscoveryTracker,
    runtime: Option<PortRuntime>,
    next_runtime_id: u64,
    url: String,
    devices: HashMap<DeviceRoute, UiDevice>,
}

impl PortModel {
    fn new(url: String) -> Self {
        Self {
            state: PortState::Idle,
            discovery_tracker: DiscoveryTracker::default(),
            runtime: None,
            next_runtime_id: 0,
            url,
            devices: HashMap::new(),
        }
    }

    fn next_runtime_token(&mut self) -> RuntimeToken {
        self.next_runtime_id = self.next_runtime_id.wrapping_add(1);
        if self.next_runtime_id == 0 {
            self.next_runtime_id = 1;
        }
        RuntimeToken(self.next_runtime_id)
    }
}

struct ActorDeps {
    app: AppHandle,
    event_tx: Sender<PumpEvent>,
    data_manager: DataManager,
    data_tx: Sender<DataMsg>,
    rpc_manager: RpcManager,
    state_view: Arc<Mutex<PortState>>,
    devices_view: Arc<RwLock<HashMap<DeviceRoute, Arc<Mutex<UiDevice>>>>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DeviceReconnectedPayload {
    port_url: String,
    children_routes: Vec<String>,
}

pub struct PortManager {
    command_tx: Sender<PortMsg>,
    pub url: String,
    pub state: Arc<Mutex<PortState>>,
    pub devices: Arc<RwLock<HashMap<DeviceRoute, Arc<Mutex<UiDevice>>>>>,
}

impl PortManager {
    pub fn new(url: String, app: AppHandle) -> Arc<Self> {
        let (command_tx, command_rx) = unbounded::<PortMsg>();
        let (event_tx, event_rx) = unbounded::<PumpEvent>();

        let data_manager = DataManager::new(DATA_BUFFER_CAPACITY);
        let data_tx = data_manager.sender();
        let rpc_manager = RpcManager::new();

        let state = Arc::new(Mutex::new(PortState::Idle));
        let devices = Arc::new(RwLock::new(HashMap::new()));

        let manager = Arc::new(Self {
            command_tx,
            url: url.clone(),
            state: state.clone(),
            devices: devices.clone(),
        });

        Self::spawn_actor(
            url,
            command_rx,
            event_rx,
            ActorDeps {
                app,
                event_tx,
                data_manager,
                data_tx,
                rpc_manager,
                state_view: state,
                devices_view: devices,
            },
        );

        let _ = manager.dispatch(PortMsg::Connect);
        manager
    }

    fn spawn_actor(
        url: String,
        command_rx: Receiver<PortMsg>,
        event_rx: Receiver<PumpEvent>,
        deps: ActorDeps,
    ) {
        thread::Builder::new()
            .name("port-manager".into())
            .spawn(move || {
                let mut model = PortModel::new(url);
                Self::sync_state_view(&model, &deps.state_view);
                Self::sync_devices_view(&model, &deps.devices_view);

                loop {
                    crossbeam::select! {
                        recv(command_rx) -> msg => {
                            match msg {
                                Ok(msg) => {
                                    if !Self::handle_command(msg, &mut model, &deps) {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        recv(event_rx) -> event => {
                            match event {
                                Ok(event) => Self::handle_pump_event(event, &mut model, &deps),
                                Err(_) => break,
                            }
                        }
                    }
                }

                Self::stop_runtime(&mut model);
                let _ = deps.data_manager.dispatch(DataMsg::Shutdown);
                let _ = deps.rpc_manager.dispatch(RpcMsg::Shutdown);
            })
            .expect("Failed to spawn port-manager thread.");
    }

    fn handle_command(msg: PortMsg, model: &mut PortModel, deps: &ActorDeps) -> bool {
        match msg {
            PortMsg::Connect | PortMsg::Rescan => {
                Self::connect_runtime(model, deps);
            }
            PortMsg::Shutdown => {
                Self::stop_runtime(model);
                Self::set_state(model, PortState::Disconnected, deps);
                let _ = deps.data_manager.dispatch(DataMsg::Shutdown);
                let _ = deps.rpc_manager.dispatch(RpcMsg::Shutdown);
                return false;
            }
            PortMsg::GetState { reply } => {
                let _ = reply.send(model.state.clone());
            }
            PortMsg::GetDevices { reply } => {
                let _ = reply.send(Self::sorted_devices(model));
            }
            PortMsg::GetDevice { route, reply } => {
                let _ = reply.send(model.devices.get(&route).cloned());
            }
            PortMsg::ExecuteRpc {
                route,
                name,
                args,
                reply,
            } => {
                let result = Self::execute_rpc_message(model, deps, route, name, args);
                let _ = reply.send(result);
            }
            PortMsg::ReadWindow { columns, n, reply } => {
                let result = match deps.data_manager.ask(|window_reply| DataMsg::ReadWindow {
                    columns,
                    n,
                    reply: window_reply,
                }) {
                    Ok(inner) => inner,
                    Err(err) => {
                        eprintln!("[{}] ReadWindow failed: {}", model.url, err);
                        Err(ReadError::NoColumnsRequested)
                    }
                };
                let _ = reply.send(result);
            }
            PortMsg::ReadTimeRange {
                columns,
                start_time,
                end_time,
                reply,
            } => {
                let result = match deps
                    .data_manager
                    .ask(|window_reply| DataMsg::ReadTimeRange {
                        columns,
                        start_time,
                        end_time,
                        reply: window_reply,
                    }) {
                    Ok(inner) => inner,
                    Err(err) => {
                        eprintln!("[{}] ReadTimeRange failed: {}", model.url, err);
                        Err(ReadError::NoColumnsRequested)
                    }
                };
                let _ = reply.send(result);
            }
            PortMsg::RefreshRpcCacheForRoutes { routes, reply } => {
                let mut status = Ok(());
                for route in routes {
                    if let Err(err) = Self::load_rpc_specs_for_route(model, deps, &route) {
                        status = Err(err);
                        break;
                    }
                }
                let _ = reply.send(status);
            }
            PortMsg::CreateSnapshot {
                id,
                columns,
                start_time,
                end_time,
                reply,
            } => {
                let result = match deps.data_manager.ask(|snap_reply| DataMsg::CreateSnapshot {
                    id,
                    columns,
                    start_time,
                    end_time,
                    reply: snap_reply,
                }) {
                    Ok(inner) => inner,
                    Err(err) => {
                        eprintln!("[{}] CreateSnapshot failed: {}", model.url, err);
                        Err(ReadError::NoColumnsRequested)
                    }
                };
                let _ = reply.send(result);
            }
            PortMsg::GetSnapshot { id, reply } => {
                let result = deps
                    .data_manager
                    .ask(|snap_reply| DataMsg::GetSnapshot {
                        id,
                        reply: snap_reply,
                    })
                    .ok()
                    .flatten();
                let _ = reply.send(result);
            }
            PortMsg::ClearSnapshot { id } => {
                let _ = deps.data_manager.dispatch(DataMsg::ClearSnapshot { id });
            }
        }

        true
    }

    fn handle_pump_event(event: PumpEvent, model: &mut PortModel, deps: &ActorDeps) {
        let active_token = model.runtime.as_ref().map(|runtime| runtime.token);

        match event {
            PumpEvent::TreeEvent { token, event } => {
                if active_token != Some(token) {
                    return;
                }

                match event {
                    TreeEvent::RouteDiscovered(route) => {
                        model.discovery_tracker.mark_discovered(route);
                        if matches!(
                            model.state,
                            PortState::Connecting
                                | PortState::Reconnecting
                                | PortState::Disconnected
                                | PortState::Idle
                        ) {
                            Self::set_state(model, PortState::Discovery, deps);
                        }
                    }
                    TreeEvent::Device { route, event } => {
                        Self::handle_device_event(model, deps, route, event);
                    }
                }
            }
            PumpEvent::Fault { token, error } => {
                if active_token != Some(token) {
                    return;
                }

                Self::stop_runtime(model);
                Self::set_state(
                    model,
                    PortState::Error(format!("Port fault: {}", error)),
                    deps,
                );
            }
            PumpEvent::Exited { token } => {
                if active_token != Some(token) {
                    return;
                }

                Self::stop_runtime(model);
                if !matches!(model.state, PortState::Error(_)) {
                    Self::set_state(model, PortState::Disconnected, deps);
                }
            }
        }
    }

    fn handle_device_event(
        model: &mut PortModel,
        deps: &ActorDeps,
        route: DeviceRoute,
        event: DeviceEvent,
    ) {
        match event {
            DeviceEvent::MetadataReady(metadata) => {
                model.discovery_tracker.mark_discovered(route.clone());
                model.discovery_tracker.mark_ready(route.clone());

                let device = Self::build_ui_device(model, &route, metadata);
                model.devices.insert(route.clone(), device);
                Self::sync_devices_view(model, &deps.devices_view);
                Self::emit_port_devices_discovered(&deps.app, model);

                if let Err(err) = Self::load_rpc_specs_for_route(model, deps, &route) {
                    eprintln!(
                        "[{}] Failed to load RPC specs for {}: {:?}",
                        model.url, route, err
                    );
                }

                if model.discovery_tracker.all_ready() {
                    Self::set_state(model, PortState::Streaming, deps);
                } else if matches!(
                    model.state,
                    PortState::Connecting | PortState::Reconnecting | PortState::Disconnected
                ) {
                    Self::set_state(model, PortState::Discovery, deps);
                }
            }
            DeviceEvent::Status(status) => match status {
                ProxyStatus::SensorDisconnected => {
                    Self::set_state(model, PortState::Disconnected, deps);
                    Self::emit_all_device_updates(&deps.app, model);
                }
                ProxyStatus::SensorReconnected => {
                    Self::set_state(model, PortState::Reconnecting, deps);
                    Self::emit_device_reconnected(&deps.app, model);
                }
                ProxyStatus::FailedToConnect => {
                    Self::set_state(
                        model,
                        PortState::Error("Failed to connect to sensor.".to_string()),
                        deps,
                    );
                }
                ProxyStatus::FailedToReconnect => {
                    Self::set_state(
                        model,
                        PortState::Error("Failed to reconnect to sensor.".to_string()),
                        deps,
                    );
                }
                ProxyStatus::Unknown(code) => {
                    Self::set_state(
                        model,
                        PortState::Error(format!("Unknown proxy status code: {}", code)),
                        deps,
                    );
                }
            },
            DeviceEvent::RpcInvalidated(method) => {
                if let Some(updated_device) = Self::invalidate_rpc_cache(model, &route, &method) {
                    Self::sync_devices_view(model, &deps.devices_view);
                    Self::emit_device_metadata_updated(&deps.app, updated_device);
                }

                let names_to_refetch: Vec<String> = match &method {
                    RpcMethod::Name(name) => model
                        .devices
                        .get(&route)
                        .and_then(|d| {
                            d.rpcs
                                .iter()
                                .find(|r| r.name == *name && r.readable)
                                .map(|r| r.name.clone())
                        })
                        .into_iter()
                        .collect(),
                    RpcMethod::Id(_) => model
                        .devices
                        .get(&route)
                        .map(|d| {
                            d.rpcs
                                .iter()
                                .filter(|r| r.readable)
                                .map(|r| r.name.clone())
                                .collect()
                        })
                        .unwrap_or_default(),
                };

                for name in names_to_refetch {
                    match Self::execute_rpc_message(
                        model,
                        deps,
                        route.clone(),
                        name.clone(),
                        Value::Null,
                    ) {
                        Ok(_) => {}
                        Err(err) => {
                            eprintln!(
                                "[{}] RPC refetch failed for {}/{}: {:?}",
                                model.url, route, name, err
                            );
                        }
                    }
                }
            }
            DeviceEvent::Heartbeat { .. } => {}
        }
    }

    fn connect_runtime(model: &mut PortModel, deps: &ActorDeps) {
        Self::stop_runtime(model);
        model.devices.clear();
        model.discovery_tracker.clear();
        Self::sync_devices_view(model, &deps.devices_view);
        Self::set_state(model, PortState::Connecting, deps);

        if let Err(err) = Self::start_runtime(model, deps) {
            Self::set_state(model, PortState::Error(err), deps);
        }
    }

    fn start_runtime(model: &mut PortModel, deps: &ActorDeps) -> Result<(), String> {
        let token = model.next_runtime_token();

        let proxy = Arc::new(proxy::Interface::new_proxy(
            &model.url,
            Some(PROXY_RECONNECT_TIMEOUT),
            None,
        ));

        let tree = DeviceTree::open(&proxy, DeviceRoute::root())
            .map_err(|e| format!("Failed to open device tree: {:?}", e))?;

        deps.rpc_manager
            .dispatch(RpcMsg::SetProxy {
                proxy: proxy.clone(),
            })
            .map_err(|e| format!("Failed to set RPC proxy: {}", e))?;

        let event_tx = deps.event_tx.clone();
        let data_tx = deps.data_tx.clone();
        let pump_join = thread::Builder::new()
            .name("port-io-pump".into())
            .spawn(move || pump_loop(token, tree, data_tx, event_tx))
            .map_err(|e| format!("Failed to spawn IO pump: {}", e))?;

        model.runtime = Some(PortRuntime {
            token,
            proxy,
            pump_join: Some(pump_join),
        });

        Ok(())
    }

    fn stop_runtime(model: &mut PortModel) {
        if let Some(mut runtime) = model.runtime.take() {
            drop(runtime.proxy);
            if let Some(join) = runtime.pump_join.take() {
                let _ = join.join();
            }
        }
    }

    fn execute_rpc_message(
        model: &mut PortModel,
        deps: &ActorDeps,
        route: DeviceRoute,
        name: String,
        args: Value,
    ) -> Result<Value, RpcError> {
        let result = deps
            .rpc_manager
            .ask(|reply| RpcMsg::Execute {
                route: route.clone(),
                name: name.clone(),
                args,
                reply,
            })
            .map_err(RpcError::AppLogic)?;
        let value = result?;

        let mut updated_device = None;
        if let Some(device) = model.devices.get_mut(&route) {
            if let Some(rpc) = device.rpcs.iter_mut().find(|rpc| rpc.name == name) {
                rpc.value = Some(value.clone());
                updated_device = Some(device.clone());
            }
        }

        if let Some(device) = updated_device {
            Self::sync_devices_view(model, &deps.devices_view);
            Self::emit_device_metadata_updated(&deps.app, device);
        }

        Ok(value)
    }

    fn load_rpc_specs_for_route(
        model: &mut PortModel,
        deps: &ActorDeps,
        route: &DeviceRoute,
    ) -> Result<(), RpcError> {
        let result = deps
            .rpc_manager
            .ask(|reply| RpcMsg::LoadSpecs {
                route: route.clone(),
                reply,
            })
            .map_err(RpcError::AppLogic)?;
        let specs = result?;

        if let Some(device) = model.devices.get_mut(route) {
            let previous_values: HashMap<String, Value> = device
                .rpcs
                .iter()
                .filter_map(|rpc| rpc.value.clone().map(|value| (rpc.name.clone(), value)))
                .collect();

            let mut rpcs: Vec<UiRpcMeta> = specs.iter().map(Self::to_ui_rpc_meta).collect();
            rpcs.sort_by(|a, b| a.name.cmp(&b.name));

            for rpc in &mut rpcs {
                if let Some(value) = previous_values.get(&rpc.name) {
                    rpc.value = Some(value.clone());
                }
            }

            device.rpcs = rpcs;
            let updated = device.clone();
            Self::sync_devices_view(model, &deps.devices_view);
            Self::emit_device_metadata_updated(&deps.app, updated);
        }

        Ok(())
    }

    fn invalidate_rpc_cache(
        model: &mut PortModel,
        route: &DeviceRoute,
        method: &RpcMethod,
    ) -> Option<UiDevice> {
        let device = model.devices.get_mut(route)?;
        let mut changed = false;

        match method {
            RpcMethod::Name(name) => {
                if let Some(rpc) = device.rpcs.iter_mut().find(|rpc| rpc.name == *name) {
                    if rpc.value.is_some() {
                        rpc.value = None;
                        changed = true;
                    }
                }
            }
            RpcMethod::Id(_) => {
                for rpc in &mut device.rpcs {
                    if rpc.value.is_some() {
                        rpc.value = None;
                        changed = true;
                    }
                }
            }
        }

        if changed {
            Some(device.clone())
        } else {
            None
        }
    }

    fn build_ui_device(
        model: &PortModel,
        route: &DeviceRoute,
        metadata: DeviceFullMetadata,
    ) -> UiDevice {
        let mut stream_pairs: Vec<(u8, _)> = metadata.streams.into_iter().collect();
        stream_pairs.sort_by_key(|(stream_id, _)| *stream_id);

        let streams: Vec<UiStream> = stream_pairs
            .into_iter()
            .map(|(_, stream_meta)| {
                let mut columns: Vec<_> = stream_meta
                    .columns
                    .iter()
                    .map(|column| column.as_ref().clone().into())
                    .collect();
                columns.sort_by_key(|column: &crate::shared::ColumnMeta| column.index);

                let segment: crate::shared::SegmentMeta =
                    stream_meta.segment.as_ref().clone().into();
                let decimation = segment.decimation.max(1) as f64;
                let effective_sampling_rate = (segment.sampling_rate as f64) / decimation;

                UiStream {
                    meta: stream_meta.stream.as_ref().clone().into(),
                    segment: Some(segment),
                    columns,
                    effective_sampling_rate,
                }
            })
            .collect();

        UiDevice {
            url: model.url.clone(),
            route: route.to_string(),
            state: model.state.clone(),
            meta: metadata.device.as_ref().clone().into(),
            streams,
            rpcs: Vec::new(),
        }
    }

    fn set_state(model: &mut PortModel, new_state: PortState, deps: &ActorDeps) {
        if model.state == new_state {
            return;
        }

        model.state = new_state.clone();
        for device in model.devices.values_mut() {
            device.state = new_state.clone();
        }

        Self::sync_state_view(model, &deps.state_view);
        Self::sync_devices_view(model, &deps.devices_view);
        Self::emit_port_state_changed(&deps.app, &model.url, &model.state);
    }

    fn sorted_devices(model: &PortModel) -> Vec<UiDevice> {
        let mut entries: Vec<(&DeviceRoute, &UiDevice)> = model.devices.iter().collect();
        entries.sort_by(|(left_route, _), (right_route, _)| left_route.cmp(right_route));
        entries
            .into_iter()
            .map(|(_, device)| device.clone())
            .collect()
    }

    fn sync_state_view(model: &PortModel, state_view: &Arc<Mutex<PortState>>) {
        if let Ok(mut state) = state_view.lock() {
            *state = model.state.clone();
        }
    }

    fn sync_devices_view(
        model: &PortModel,
        devices_view: &Arc<RwLock<HashMap<DeviceRoute, Arc<Mutex<UiDevice>>>>>,
    ) {
        let mut next = HashMap::new();
        for (route, device) in &model.devices {
            next.insert(route.clone(), Arc::new(Mutex::new(device.clone())));
        }

        if let Ok(mut devices) = devices_view.write() {
            *devices = next;
        }
    }

    fn emit_port_state_changed(app: &AppHandle, url: &str, state: &PortState) {
        if let Err(err) = app.emit("port-state-changed", (url.to_string(), state.clone())) {
            eprintln!("[{}] Failed to emit port-state-changed: {}", url, err);
        }
    }

    fn emit_port_devices_discovered(app: &AppHandle, model: &PortModel) {
        let devices = Self::sorted_devices(model);
        if let Err(err) = app.emit("port-devices-discovered", devices) {
            eprintln!(
                "[{}] Failed to emit port-devices-discovered: {}",
                model.url, err
            );
        }
    }

    fn emit_device_metadata_updated(app: &AppHandle, device: UiDevice) {
        if let Err(err) = app.emit("device-metadata-updated", device.clone()) {
            eprintln!(
                "[{}:{}] Failed to emit device-metadata-updated: {}",
                device.url, device.route, err
            );
        }
    }

    fn emit_all_device_updates(app: &AppHandle, model: &PortModel) {
        for device in Self::sorted_devices(model) {
            Self::emit_device_metadata_updated(app, device);
        }
    }

    fn emit_device_reconnected(app: &AppHandle, model: &PortModel) {
        let mut children_routes: Vec<String> = model
            .devices
            .keys()
            .filter(|route| route.len() > 0)
            .map(|route| route.to_string())
            .collect();
        children_routes.sort();

        let payload = DeviceReconnectedPayload {
            port_url: model.url.clone(),
            children_routes,
        };

        if let Err(err) = app.emit("device-reconnected", payload) {
            eprintln!("[{}] Failed to emit device-reconnected: {}", model.url, err);
        }
    }

    fn to_ui_rpc_meta(meta: &TwinleafRpcMeta) -> UiRpcMeta {
        UiRpcMeta {
            name: meta.full_name.clone(),
            size: meta.size_bytes().unwrap_or(0),
            permissions: meta.perm_str(),
            arg_type: meta.type_str(),
            readable: meta.readable,
            writable: meta.writable,
            persistent: meta.persistent,
            unknown: meta.is_unknown(),
            value: None,
        }
    }

    pub fn dispatch(&self, msg: PortMsg) -> Result<(), String> {
        self.command_tx
            .send(msg)
            .map_err(|_| "PortManager actor is unavailable.".to_string())
    }

    pub fn ask<R>(&self, build: impl FnOnce(Sender<R>) -> PortMsg) -> Result<R, String>
    where
        R: Send + 'static,
    {
        let (reply_tx, reply_rx) = bounded(1);
        self.command_tx
            .send(build(reply_tx))
            .map_err(|_| "PortManager actor is unavailable.".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "PortManager actor did not respond.".to_string())
    }

    pub fn connect(&self) {
        let _ = self.dispatch(PortMsg::Connect);
    }

    pub fn rescan(&self) {
        let _ = self.dispatch(PortMsg::Rescan);
    }

    pub fn shutdown(&self) {
        let _ = self.dispatch(PortMsg::Shutdown);
    }

    pub async fn execute_rpc(
        &self,
        device_route: &str,
        name: &str,
        args: Option<Value>,
    ) -> Result<Value, RpcError> {
        let route = DeviceRoute::from_str(device_route)
            .map_err(|_| RpcError::AppLogic(format!("Invalid device route '{}'.", device_route)))?;

        self.ask(|reply| PortMsg::ExecuteRpc {
            route,
            name: name.to_string(),
            args: args.unwrap_or(Value::Null),
            reply,
        })
        .map_err(RpcError::AppLogic)?
    }

    pub fn refresh_rpc_cache_for_routes(&self, routes: &[DeviceRoute]) -> Result<(), RpcError> {
        self.ask(|reply| PortMsg::RefreshRpcCacheForRoutes {
            routes: routes.to_vec(),
            reply,
        })
        .map_err(RpcError::AppLogic)?
    }
}

fn pump_loop(
    token: RuntimeToken,
    mut tree: DeviceTree,
    data_tx: Sender<DataMsg>,
    event_tx: Sender<PumpEvent>,
) {
    loop {
        match tree.next_item() {
            Ok(TreeItem::Sample(sample, route)) => {
                if data_tx.send(DataMsg::Ingest { sample, route }).is_err() {
                    break;
                }
            }
            Ok(TreeItem::Event(event)) => {
                if event_tx
                    .send(PumpEvent::TreeEvent { token, event })
                    .is_err()
                {
                    return;
                }
            }
            Err(err) => {
                let _ = event_tx.send(PumpEvent::Fault {
                    token,
                    error: format!("{:?}", err),
                });
                break;
            }
        }
    }

    let _ = event_tx.send(PumpEvent::Exited { token });
}
