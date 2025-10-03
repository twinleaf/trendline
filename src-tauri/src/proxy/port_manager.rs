use crate::shared::{
    ColumnMeta, DataColumnId, DeviceMeta, Point, PortState, RpcError, RpcMeta, UiDevice, UiStream,
};
use crate::state::capture::{CaptureCommand, CaptureState, SessionId};
use crate::state::proxy_register::ProxyRegister;
use crate::util::{self, parse_arg_type_and_size, parse_permissions_string};
use crossbeam::channel::{Sender, TrySendError};
use crossbeam::select;
use serde_json::{json, Value};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    collections::{HashMap, HashSet},
    panic,
    sync::{Arc, Mutex, RwLock},
    thread,
    time::{Duration, Instant},
};
use tauri::menu::MenuItemKind;
use tauri::{async_runtime, Emitter, Manager};
use twinleaf::{
    tio::{
        proto::DeviceRoute,
        proxy::{self, Event},
    },
    Device,
};

pub struct DebugCounters {
    pub polls: AtomicUsize,
    pub samples_received: AtomicUsize,
    pub points_inserted: AtomicUsize,
    pub dropped_batches: AtomicUsize,
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
    pub devices: RwLock<HashMap<DeviceRoute, Arc<Mutex<(Device, UiDevice)>>>>,
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

    fn spawn_thread(self_: Arc<Self>, command_rx: crossbeam::channel::Receiver<PortCommand>) {
        thread::Builder::new()
            .name(format!("port-{}", self_.url))
            .spawn(move || {
                let (status_tx, status_rx) = crossbeam::channel::unbounded();
                let ticker = crossbeam::channel::tick(Duration::from_millis(10));

                let mut last_debug_print = Instant::now();

                'lifecycle: loop {
                    let current_state = self_.state.lock().unwrap().clone();
                    let has_proxy = self_.proxy.lock().unwrap().is_some();

                    if !has_proxy && current_state == PortState::Idle {
                        self_.set_state(PortState::Connecting);

                        println!("[{}] Attempting to establish proxy connection...", self_.url);
                        let proxy_if = Arc::new(proxy::Interface::new_proxy(
                            &self_.url,
                            Some(Duration::from_secs(30)),
                            Some(status_tx.clone()),
                        ));
                        *self_.proxy.lock().unwrap() = Some(proxy_if);
                    }

                    select! {
                        recv(status_rx) -> event => match event {
                            Ok(Event::SensorConnected) | Ok(Event::SensorReconnected) => {
                                *self_.connection_retries.lock().unwrap() = 0;

                                println!("[{}] Connection established.", self_.url);

                                if let Some(proxy_if) = self_.proxy.lock().unwrap().clone() {
                                    if self_.discover_devices(&proxy_if).is_ok() {
                                        println!("[{}] Discovery finished, beginning to stream data.", self_.url);
                                        let registry = self_.app.state::<Arc<ProxyRegister>>();
                                        if let Some(keys) = registry.active_selections.get(&self_.url) {
                                            println!("[{}] Re-applying cached selection of {} columns.", self_.url, keys.len());
                                            let capture_state = self_.app.state::<CaptureState>();
                                             let command = crate::state::capture::CaptureCommand::SetActiveColumns {
                                                port_url: self_.url.clone(),
                                                keys_for_port: keys.value().clone(),
                                            };

                                            if let Err(e) = capture_state.inner.command_tx.send(command) {
                                                eprintln!("[{}] Failed to send SetActiveColumns command: {}", self_.url, e);
                                            }
                                        }
                                        self_.set_state(PortState::Streaming);
                                    } else {
                                        let err_msg = "Discovery failed after connection.".to_string();
                                        eprintln!("[{}] {}", self_.url, err_msg);
                                        self_.set_state(PortState::Error(err_msg));
                                    }
                                }
                            },
                            Ok(Event::SensorDisconnected) => {
                                println!("[{}] Connection lost. Proxy is auto-reconnecting...", self_.url);
                                self_.set_state(PortState::Reconnecting);

                                let devices_map = self_.devices.read().unwrap();
                                for device_entry in devices_map.values() {
                                    let mut device_tuple = device_entry.lock().unwrap();
                                    let (_, ui_device) = &mut *device_tuple;
                                    ui_device.state = PortState::Reconnecting;
                                    self_.app.emit("device-metadata-updated", ui_device.clone()).unwrap();
                                }
                            },
                            Ok(Event::FailedToConnect) => {
                                const MAX_RETRIES: u32 = 10;
                                let mut retries = self_.connection_retries.lock().unwrap();
                                *retries += 1;

                                if *retries >= MAX_RETRIES {
                                    let err_msg = format!("Failed to connect after {} attempts. Giving up.", MAX_RETRIES);
                                    eprintln!("[{}] {}", self_.url, err_msg);
                                    self_.set_state(PortState::Error(err_msg));
                                    *self_.proxy.lock().unwrap() = None;
                                    *retries = 0;
                                } else {
                                    println!("[{}] Failed to connect (attempt {}/{}). Retrying in 2s...", self_.url, *retries, MAX_RETRIES);
                                    self_.set_state(PortState::Idle);
                                    *self_.proxy.lock().unwrap() = None;
                                    thread::sleep(Duration::from_secs(2));
                                }
                            },
                            Ok(Event::FailedToReconnect) => {
                                let err_msg = "Proxy failed to reconnect after timeout.".to_string();
                                eprintln!("[{}] {}", self_.url, err_msg);
                                self_.set_state(PortState::Error(err_msg));
                                *self_.proxy.lock().unwrap() = None;
                            },
                            Ok(Event::FatalError(e)) => {
                                let err_msg = format!("Fatal proxy error: {:?}", e);
                                eprintln!("[{}] {}", self_.url, err_msg);
                                self_.set_state(PortState::Error(err_msg));
                                break 'lifecycle;
                            },
                            Ok(Event::Exiting) => {
                                println!("[{}] Proxy is exiting.", self_.url);
                                break 'lifecycle;
                            },
                            Err(_) => {
                                eprintln!("[{}] Status channel broke. Shutting down.", self_.url);
                                break 'lifecycle;
                            }
                            _ => { /* All other events are ignored */ }
                        },

                        recv(command_rx) -> command => match command {
                            Ok(PortCommand::Shutdown) => {
                                println!("[{}] Shutdown command received.", self_.url);
                                break 'lifecycle;
                            },
                            Ok(PortCommand::AttemptConnection) => {
                                println!("[{}] Manual connection attempt triggered.", self_.url);
                                let current_state = self_.state.lock().unwrap().clone();
                                if matches!(current_state, PortState::Error(_)) {
                                    self_.set_state(PortState::Idle);
                                    *self_.proxy.lock().unwrap() = None;
                                }
                            },
                            Ok(PortCommand::RescanDevices) => {
                                println!("[{}] Manual device rescan triggered.", self_.url);
                                if let Some(proxy_if) = self_.proxy.lock().unwrap().clone() {
                                    let prev = self_.state.lock().unwrap().clone();
                                    self_.set_state(PortState::Discovery);

                                    if let Err(e) = self_.discover_devices(&proxy_if) {
                                        eprintln!("[{}] Rescan failed: {:?}", self_.url, e);
                                    }
                                    self_.set_state(prev);
                                } else {
                                    let current_state = self_.state.lock().unwrap().clone();
                                    if matches!(current_state, PortState::Error(_)) {
                                        self_.set_state(PortState::Idle);
                                        *self_.proxy.lock().unwrap() = None;
                                    }
                                }
                            }
                            Err(_) => {
                                eprintln!("[{}] Command channel broke. Shutting down.", self_.url);
                                break 'lifecycle;
                            }
                        },

                        recv(ticker) -> _ => {
                            let current_state = self_.state.lock().unwrap().clone();
                            if current_state == PortState::Streaming {
                                self_.poll_device_data();
                            }
                            if last_debug_print.elapsed() > Duration::from_secs(30) {
                                let polls  = self_.counters.polls.swap(0, Ordering::Relaxed);
                                let points = self_.counters.points_inserted.swap(0, Ordering::Relaxed);
                                let batch  = self_.counters.dropped_batches.swap(0, Ordering::Relaxed);

                                let state = self_.state.lock().unwrap().clone();

                                if !matches!(state, PortState::Error(_)) {
                                    println!(
                                        "[{}] Heartbeat (30s): State={:?}, Polls={}, PointsIns={}, DroppedBatches={}",
                                        self_.url, state, polls, points, batch
                                    );
                                }

                                last_debug_print = Instant::now();
                            }
                        }
                    }
                }

                println!("[{}] Port manager thread cleaning up and shutting down.", self_.url);
                if let Some(proxy) = self_.proxy.lock().unwrap().take() {
                    drop(proxy);
                }
                self_.devices.write().unwrap().clear();
                if !matches!(*self_.state.lock().unwrap(), PortState::Error(_)) {
                    self_.set_state(PortState::Disconnected);
                }
            })
            .expect("Failed to spawn PortManager thread.");
    }

    fn discover_devices(&self, proxy_if: &Arc<proxy::Interface>) -> Result<(), proxy::PortError> {
        // Maybe change to tree_probe(), but some sensors only emit data on StreamData which are not forwarded to probe routes
        let discovery_port = proxy_if.tree_full()?;
        let mut discovered_routes = HashSet::new();
        let mut discovery_deadline = Instant::now() + Duration::from_secs(2);

        self.set_state(PortState::Discovery);
        println!("[{}] Listening for device routes...", self.url);

        while Instant::now() < discovery_deadline {
            if let Ok(pkt) = discovery_port
                .receiver()
                .recv_timeout(Duration::from_millis(100))
            {
                if discovered_routes.insert(pkt.routing) {
                    discovery_deadline = Instant::now() + Duration::from_secs(2);
                }
            }
        }
        drop(discovery_port);
        println!("[{}] Found new routes: {:?}", self.url, discovered_routes);
        if discovered_routes.is_empty() {
            return Ok(());
        }

        let mut discovered_info = Vec::new();
        for route in discovered_routes {
            println!("[{}] Initializing device on route '{}'...", self.url, route);
            match self.initialize_ui_device(proxy_if, &route) {
                Ok(ui_dev) => {
                    discovered_info.push((route, ui_dev));
                }
                Err(e) => {
                    eprintln!(
                        "[{}] Failed to build UI for device on route '{}': {:?}",
                        self.url, route, e
                    );
                }
            }
        }

        if discovered_info.is_empty() {
            return Ok(());
        }

        let mut devices = self.devices.write().unwrap();
        let mut discovered_ui_devices_for_event = Vec::new();
        for (route, ui_dev) in discovered_info {
            let data_device = Device::open(proxy_if, route.clone());

            self.update_capture_state_with_stream_metadata(&route, &ui_dev.streams);

            discovered_ui_devices_for_event.push(ui_dev.clone());
            devices.insert(route, Arc::new(Mutex::new((data_device, ui_dev))));
        }

        if !discovered_ui_devices_for_event.is_empty() {
            println!(
                "[{}] Publishing batch of {} devices",
                self.url,
                discovered_ui_devices_for_event.len()
            );
            self.app
                .emit("port-devices-discovered", discovered_ui_devices_for_event)
                .unwrap();
        }

        Ok(())
    }

    pub fn initialize_ui_device(
        &self,
        proxy_if: &Arc<proxy::Interface>,
        route: &DeviceRoute,
    ) -> Result<UiDevice, proxy::PortError> {
        let rpc_port = proxy_if.device_rpc(route.clone())?;
        let mut temp_rpc_device = Device::new(rpc_port);
        let rpcs = self.fetch_rpcs(&mut temp_rpc_device);

        let mut temp_data_device = Device::open(proxy_if, route.clone());
        let (meta, streams) = self.fetch_metadata(&mut temp_data_device);

        println!(
            "[{}]   -> Fetched {} streams and {} RPCs for '{}'",
            self.url,
            streams.len(),
            rpcs.len(),
            route
        );

        Ok(UiDevice {
            url: self.url.clone(),
            route: route.to_string(),
            state: self.state.lock().unwrap().clone(),
            meta,
            streams,
            rpcs,
        })
    }

    fn fetch_rpcs(&self, rpc_device: &mut Device) -> Vec<RpcMeta> {
        println!("[{}]   -> Fetching RPCs...", self.url);
        let mut rpc_metas = Vec::new();
        let rpc_count: u16 = match rpc_device.get("rpc.listinfo") {
            Ok(count) => count,
            Err(_) => return rpc_metas,
        };

        for rpc_id in 0..rpc_count {
            if let Ok((meta_bits, name)) =
                rpc_device.rpc::<u16, (u16, String)>("rpc.listinfo", rpc_id)
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
                    let rpc_result = panic::catch_unwind(AssertUnwindSafe(|| {
                        match arg_type.as_str() {
                            "u8" => rpc_device.get::<u8>(&name).map(|v| json!(v)),
                            "u16" => rpc_device.get::<u16>(&name).map(|v| json!(v)),
                            "u32" => rpc_device.get::<u32>(&name).map(|v| json!(v)),
                            "u64" => rpc_device.get::<u64>(&name).map(|v| json!(v)),
                            "i8" => rpc_device.get::<i8>(&name).map(|v| json!(v)),
                            "i16" => rpc_device.get::<i16>(&name).map(|v| json!(v)),
                            "i32" => rpc_device.get::<i32>(&name).map(|v| json!(v)),
                            "i64" => rpc_device.get::<i64>(&name).map(|v| json!(v)),
                            "f32" => rpc_device.get::<f32>(&name).map(|v| json!(v)),
                            "f64" => rpc_device.get::<f64>(&name).map(|v| json!(v)),
                            "string" => rpc_device.get::<String>(&name).map(|v| json!(v)),
                            s if s.starts_with("string<") => {
                                rpc_device.get::<String>(&name).map(|v| json!(v))
                            }
                            _ => {
                                // Fallback to original method for unhandled types
                                match rpc_device.raw_rpc(&name, &[]) {
                                    Ok(reply_bytes) => {
                                        Ok(util::bytes_to_json_value(&reply_bytes, &arg_type)
                                            .unwrap_or(Value::Null))
                                    }
                                    Err(e) => Err(e),
                                }
                            }
                        }
                    }));

                    if let Ok(call_result) = rpc_result {
                        match call_result {
                            Ok(value) => meta.value = Some(value),
                            Err(proxy::RpcError::TypeError) => {
                                meta.value = Some(Value::Null);
                            }
                            Err(_) => {}
                        }
                    }
                }
                rpc_metas.push(meta);
            }
        }
        rpc_metas
    }

    fn fetch_metadata(&self, data_device: &mut Device) -> (DeviceMeta, Vec<UiStream>) {
        println!("[{}]   -> Fetching metadata...", self.url);
        let metadata = data_device.get_metadata();

        let device_meta = DeviceMeta::from((*metadata.device).clone());

        let mut sorted_streams: Vec<_> = metadata.streams.values().collect();
        sorted_streams.sort_by_key(|s| s.stream.stream_id);

        let ui_streams: Vec<UiStream> = sorted_streams
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
            .collect();
        println!("[{}]   -> Fetched metadata!", self.url);
        (device_meta, ui_streams)
    }

    fn set_state(&self, new_state: PortState) {
        *self.state.lock().unwrap() = new_state.clone();
        self.app
            .emit("port-state-changed", (self.url.clone(), new_state.clone()))
            .unwrap();
        println!("[{}] Emit new port state {:?}", self.url, new_state);

        if let Some(window) = self.app.get_webview_window("main") {
            if let Some(menu) = window.menu() {
                let is_connected = new_state == PortState::Streaming;

                // Update items in the "File" menu
                if let Some(file_menu) = util::find_submenu_by_text(&menu, "File") {
                    // Find the specific item by its ID within the submenu
                    if let Some(MenuItemKind::MenuItem(item)) = file_menu.get("save_recording") {
                        item.set_enabled(is_connected).unwrap();
                    }
                }

                // Update items in the "Edit" menu
                if let Some(edit_menu) = util::find_submenu_by_text(&menu, "Edit") {
                    if let Some(MenuItemKind::MenuItem(item)) = edit_menu.get("clear_session") {
                        item.set_enabled(!is_connected).unwrap();
                    }
                }

                // Find the "Device" menu once and update all its items
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

    fn poll_device_data(self: &Arc<Self>) {
        self.counters.polls.fetch_add(1, Ordering::Relaxed);
        let poll_instant = Instant::now();

        let mut refresh_needed = HashSet::new();
        let mut reinit_needed = HashSet::new();

        struct BatchedEntry {
            points: Vec<Point>,
            sample_numbers: Vec<u32>,
        }

        let mut batched: HashMap<(DataColumnId, SessionId), BatchedEntry> = HashMap::new();

        {
            let devices_map = self.devices.read().unwrap();
            for (route, entry) in devices_map.iter() {
                let mut tuple = entry.lock().unwrap();
                let (device, _) = &mut *tuple;

                let result = panic::catch_unwind(AssertUnwindSafe(|| device.drain()));

                match result {
                    Ok(samples) => {
                        for sample in samples {
                            let sid = sample.device.session_id;
                            let sample_number = sample.n;

                            for col in &sample.columns {
                                let val = match col.value {
                                    twinleaf::data::ColumnData::Int(i) => i as f64,
                                    twinleaf::data::ColumnData::UInt(u) => u as f64,
                                    twinleaf::data::ColumnData::Float(f) => f,
                                    _ => continue,
                                };

                                let key = DataColumnId {
                                    port_url: self.url.clone(),
                                    device_route: route.clone(),
                                    stream_id: sample.stream.stream_id,
                                    column_index: col.desc.index,
                                };

                                let e = batched
                                    .entry((key, sid))
                                    .or_insert_with(|| BatchedEntry {
                                        points: Vec::new(),
                                        sample_numbers: Vec::new(),
                                    });

                                e.points.push(Point {
                                    x: sample.timestamp_end(),
                                    y: val,
                                });
                                e.sample_numbers.push(sample_number);
                            }

                            if sample.meta_changed || sample.segment_changed {
                                refresh_needed.insert(route.clone());
                            }
                        }
                    }
                    Err(_) => {
                        println!(
                            "[{}] Device '{}' unresponsive. Attempting to reinstate handler.",
                            self.url, route
                        );
                        reinit_needed.insert(route.clone());
                    }
                }
            }
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
                Err(TrySendError::Full(_cmd)) => {
                    self.counters
                        .dropped_batches
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(TrySendError::Disconnected(_cmd)) => {
                    // capture thread died; bail
                    break;
                }
            }
        }

        if !reinit_needed.is_empty() {
            if let Some(proxy_if) = self.proxy.lock().unwrap().clone() {
                let devices_map = self.devices.read().unwrap();
                for route in reinit_needed {
                    if let Some(device_entry) = devices_map.get(&route) {
                        let mut tuple = device_entry.lock().unwrap();
                        tuple.0 = Device::open(&proxy_if, route.clone());
                        println!(
                            "[{}] Re-initialized TIO device for route '{}'.",
                            self.url, route
                        );
                    }
                }
            }
        }

        if refresh_needed.is_empty() {
            return;
        }

        let devices_map = self.devices.read().unwrap();
        for route in refresh_needed {
            if let Some(device_entry) = devices_map.get(&route) {
                let device_arc_clone = device_entry.clone();
                let self_clone = self.clone();

                async_runtime::spawn_blocking(move || {
                    println!(
                        "[{}] Spawning background task to refresh metadata for route '{}'",
                        self_clone.url, route
                    );

                    let mut device_tuple = device_arc_clone.lock().unwrap();
                    let (data_device, ui_device) = &mut *device_tuple;
                    let (new_meta, new_streams) = self_clone.fetch_metadata(data_device);

                    self_clone.update_capture_state_with_stream_metadata(&route, &new_streams);

                    ui_device.meta = new_meta;
                    ui_device.streams = new_streams;

                    if let Err(e) = self_clone
                        .app
                        .emit("device-metadata-updated", ui_device.clone())
                    {
                        eprintln!(
                            "[{}] Failed to emit device-metadata-updated event: {}",
                            self_clone.url, e
                        );
                    }
                });
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

        let proxy_if = self
            .proxy
            .lock()
            .map_err(|_| RpcError::AppLogic("Proxy lock was poisoned.".to_string()))?
            .clone()
            .ok_or_else(|| RpcError::AppLogic("Proxy interface not available.".to_string()))?;

        let rpc_meta = {
            let devices_map = self.devices.read().map_err(|_| {
                RpcError::AppLogic("Device cache read lock was poisoned.".to_string())
            })?;

            let device_entry = devices_map.get(&route).ok_or_else(|| {
                RpcError::AppLogic(format!("Device '{}' not found in cache.", route))
            })?;

            let device_tuple = device_entry.lock().map_err(|_| {
                RpcError::AppLogic(format!("Device lock for '{}' was poisoned.", route))
            })?;

            let (_data_device, ui_device) = &*device_tuple;

            ui_device
                .rpcs
                .iter()
                .find(|r| r.name == name)
                .cloned()
                .ok_or_else(|| RpcError::AppLogic(format!("RPC '{}' not found.", name)))?
        };

        let rpc_task_result = {
            let route_clone = route.clone();
            let name_clone = name.to_string();
            let args_clone = args.clone();

            tauri::async_runtime::spawn_blocking(move || -> Result<Value, RpcError> {
                let rpc_port = proxy_if.device_rpc(route_clone)?;
                let mut device = Device::new(rpc_port);
                let arg_bytes = util::json_to_bytes(args_clone, &rpc_meta.arg_type)?;
                let reply_bytes = device.raw_rpc(&name_clone, &arg_bytes)?;
                util::bytes_to_json_value(&reply_bytes, &rpc_meta.arg_type)
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
                let mut device_tuple = device_entry.lock().map_err(|_| {
                    RpcError::AppLogic(format!("Device lock for '{}' was poisoned.", route))
                })?;

                let (_, ui_device) = &mut *device_tuple;
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
