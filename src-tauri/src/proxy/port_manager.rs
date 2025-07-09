use crate::state::capture::{CaptureState, DataColumnId, Point};
use crate::shared::{ColumnMeta, DeviceMeta, PortState, RpcError, RpcMeta, UiDevice, UiStream};
use crate::util::{self, parse_arg_type_and_size, parse_permissions_string};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
    thread,
};
use serde_json::Value;
use tauri::menu::MenuItemKind;
use tauri::{Emitter, Manager};
use tokio::sync::oneshot;
use twinleaf::{
    data::Sample,
    tio::{
        proto::{DeviceRoute},
        proxy::{self, Event},
    },
    Device,
};


// Commands that can be sent from other parts of the application into the PortManager's thread.
#[derive(Debug)]
pub enum PortCommand {
    ExecuteRpc {
        route: DeviceRoute,
        name: String,
        args: Option<Value>, 
        responder: oneshot::Sender<Result<Value, RpcError>>,
    },
    Shutdown,
}

pub struct PortManager {
    pub url: String,
    pub state: Mutex<PortState>,
    pub proxy: Mutex<Option<Arc<proxy::Interface>>>,
    pub devices: Mutex<HashMap<DeviceRoute, (Device, UiDevice)>>,
    pub command_tx: crossbeam::channel::Sender<PortCommand>,
    pub app: tauri::AppHandle,
    pub capture: CaptureState,
}

impl PortManager {
    pub fn new(url: String, app: tauri::AppHandle, capture: CaptureState) -> Arc<Self> {
        let (command_tx, command_rx) = crossbeam::channel::unbounded();

        let pm = Arc::new(Self {
            url,
            state: Mutex::new(PortState::Idle),
            proxy: Mutex::new(None),
            devices: Mutex::new(HashMap::new()),
            command_tx,
            app,
            capture,
        });

        Self::spawn_thread(pm.clone(), command_rx);
        pm
    }

    /// Spawns the dedicated thread that manages the connection lifecycle.
    fn spawn_thread(
        self_: Arc<Self>,
        command_rx: crossbeam::channel::Receiver<PortCommand>,
    ) {
        thread::Builder::new()
            .name(format!("port-{}", self_.url))
            .spawn(move || {

                'lifecycle: loop {
                    // --- PHASE 1: CONNECTION ---
                    self_.set_state(PortState::Connecting);
                    println!("[{}] Establishing proxy...", self_.url);

                    let (status_tx, status_rx) = crossbeam::channel::unbounded();
                    let reconnect_timeout = Some(Duration::from_secs(5));

                    let proxy_if = Arc::new(proxy::Interface::new_proxy(
                        &self_.url,
                        reconnect_timeout,
                        Some(status_tx),
                    ));
                    
                    
                    // Store the interface so other parts of the app could potentially use it.
                    *self_.proxy.lock().unwrap() = Some(proxy_if.clone());

                    // Wait for the connection to be established.
                    match Self::wait_for_connection(&status_rx) {
                        Ok(_) => {
                             println!("[{}] Proxy connected.", self_.url);
                        },
                        Err(_) => {
                            println!("[{}] Failed to connect. Will retry.", self_.url);
                            self_.set_state(PortState::Disconnected);
                            thread::sleep(Duration::from_secs(5));
                            continue 'lifecycle; // Restart the connection process
                        }
                    }

                    // --- PHASE 2: DEVICE DISCOVERY ---
                    self_.set_state(PortState::Discovery);
                    println!("[{}] Starting device discovery on proxy stream...", self_.url);
                    if let Err(e) = self_.discover_devices(&proxy_if) {
                        eprintln!("[{}] Discovery failed: {:?}. Retrying.", self_.url, e);
                        continue 'lifecycle;
                    }
                    println!("[{}] Discovery finished.", self_.url);

                    // --- PHASE 3: STREAMING & COMMAND HANDLING ---
                    self_.set_state(PortState::Streaming);
                    println!("[{}] Entering streaming mode.", self_.url);
                    loop {
                        // Check for status changes from the proxy (e.g., disconnects)
                        if let Ok(event) = status_rx.try_recv() {
                            match event {
                                Event::SensorDisconnected => {
                                    println!("[{}] Sensor disconnected. Attempting to reconnect...", self_.url);
                                    self_.set_state(PortState::Reconnecting);
                                    continue 'lifecycle;
                                }
                                Event::Exiting | Event::FatalError(_) => {
                                    eprintln!("[{}] Fatal proxy error. Shutting down port.", self_.url);
                                    self_.set_state(PortState::Disconnected);
                                    break 'lifecycle;
                                }
                                _ => { /* Other events can be logged if needed */ }
                            }
                        }
                        
                        // Lock the devices map once to handle all device-related work for this loop iteration.
                        let mut devices = self_.devices.lock().unwrap();

                        // Check for incoming commands from the app
                        if let Ok(command) = command_rx.try_recv() {
                            match command {
                                PortCommand::Shutdown => {
                                    println!("[{}] Shutdown command received.", self_.url);
                                    break 'lifecycle;
                                },
                                PortCommand::ExecuteRpc { route, name, args, responder } => {
                                    let result: Result<Value, RpcError> = (|| {
                                        
                                        let ui_dev = devices.get(&route)
                                            .map(|(_device, ui_device)| ui_device.clone())
                                            .ok_or_else(|| RpcError::AppLogic(format!("Device with route '{}' not found.", route)))?;
                                        
                                        let rpc_meta = ui_dev.rpcs.iter().find(|r| r.name == name)
                                            .ok_or_else(|| RpcError::AppLogic(format!("RPC '{}' not found in metadata.", name)))?;

                                        let device = &mut devices.get_mut(&route).unwrap().0;

                                        let arg_bytes = util::json_to_bytes(args, &rpc_meta.arg_type)
                                            .map_err(|msg| RpcError::AppLogic(msg))?;

                                        let reply_bytes = device.raw_rpc(&name, &arg_bytes)
                                            .map_err(RpcError::from)?;
                                        
                                        let reply_value = util::bytes_to_json_value(&reply_bytes, &rpc_meta.arg_type)
                                                            .unwrap_or(Value::Null);

                                        if let Some((_d, ui_d)) = devices.get_mut(&route) {
                                            if let Some(rpc) = ui_d.rpcs.iter_mut().find(|r| r.name == name) {
                                                rpc.value = Some(reply_value.clone());
                                            }
                                        }

                                        Ok(reply_value)
                                    })();
                                    let _ = responder.send(result);
                                }
                            }
                        }

                        // Poll each device for new samples
                        for (route, (device, _ui_device)) in devices.iter_mut() {
                            while let Some(sample) = device.try_next() {
                                self_.process_incoming_sample(&sample, route);
                            }
                        }
                        
                        drop(devices);

                        // Don't busy-wait. A small sleep is crucial.
                        thread::sleep(Duration::from_millis(1));
                    }
                }
                
                // --- SHUTDOWN ---
                println!("[{}] Port manager thread finished.", self_.url);
                self_.devices.lock().unwrap().clear();
                *self_.proxy.lock().unwrap() = None;
                self_.set_state(PortState::Disconnected);
            })
            .expect("Failed to spawn port manager thread");
    }
    
    fn discover_devices(&self, proxy_if: &proxy::Interface) -> Result<(), proxy::PortError> {
        let discovery_port = proxy_if.tree_probe()?;
        let mut discovered_routes = HashSet::new();
        let discovery_deadline = Instant::now() + Duration::from_secs(2);

        println!("[{}] Listening for device routes...", self.url);
        while Instant::now() < discovery_deadline {
            if let Ok(pkt) = discovery_port.receiver().recv_timeout(Duration::from_millis(100)) {
                discovered_routes.insert(pkt.routing);
            }
        }
        drop(discovery_port);
        println!("[{}] Found routes: {:?}", self.url, discovered_routes);


        let mut discovered_ui_devices: Vec<UiDevice> = Vec::new();
        
        let mut devices = self.devices.lock().unwrap();
        for route in discovered_routes {
            if devices.contains_key(&route) {
                continue;
            }

            println!("[{}] Initializing device on route '{}'...", self.url, route);
            // Create a dedicated port for this device to get metadata and stream data.
            let dev_port = proxy_if.device_full(route.clone())?;
            let mut device = Device::new(dev_port);
            let metadata = device.get_metadata(); 
            println!("[{}]   -> Metadata for '{}': {:?}", self.url, route, metadata.device.name);

            let mut rpc_list = Self::list_rpcs_for_device(&mut device);
            for rpc_meta in &mut rpc_list {
                if rpc_meta.readable {
                    if let Ok(reply_bytes) = device.raw_rpc(&rpc_meta.name, &[]) {
                        // Update the value in place
                        rpc_meta.value = util::bytes_to_json_value(&reply_bytes, &rpc_meta.arg_type);
                    }
                }
            }
            println!("[{}]   -> Fetched {} RPCs for '{}'", self.url, rpc_list.len(), route);
            
            let mut sorted_streams_meta: Vec<_> = metadata.streams.values().collect();
            sorted_streams_meta.sort_by_key(|s| s.stream.stream_id);
            
            let ui_streams: Vec<UiStream> = sorted_streams_meta.into_iter().map(|device_stream_meta| {
                    let lib_stream_meta = &*device_stream_meta.stream;
                    let lib_segment_meta = &*device_stream_meta.segment;
                    
                    let mut ui_columns: Vec<ColumnMeta> = device_stream_meta.columns.iter().map(|lib_column_arc| {
                        ColumnMeta::from((**lib_column_arc).clone())
                    }).collect();
                    
                    ui_columns.sort_by_key(|c| c.index);

                    UiStream {
                        meta: (*lib_stream_meta).clone().into(),
                        segment: Some((*lib_segment_meta).clone().into()), 
                        columns: ui_columns,
                    }
            }).collect();

            let ui_dev = UiDevice{
                url: self.url.clone(),
                route: route.to_string(),
                state: self.state.lock().unwrap().clone(),
                meta: DeviceMeta::from((*metadata.device).clone()),
                streams: ui_streams,
                rpcs: rpc_list,
            };

            discovered_ui_devices.push(ui_dev.clone());
            devices.insert(route, (device, ui_dev));
        }

        if !discovered_ui_devices.is_empty() {
            println!("[{}] -> Publishing batch of {} devices", self.url, discovered_ui_devices.len());
            self.app.emit("port-devices-discovered", discovered_ui_devices).unwrap();
        }

        Ok(())
    }

    /// Gracefully shuts down the manager's thread.
    pub fn shutdown(&self) {
        let _ = self.command_tx.send(PortCommand::Shutdown);
    }
    
    fn set_state(&self, new_state: PortState) {
        *self.state.lock().unwrap() = new_state.clone();
        self.app
            .emit("port-state-changed", (self.url.clone(), new_state.clone()))
            .unwrap();


        if let Some(window) = self.app.get_webview_window("main") {
            if let Some(menu) = window.menu() {
                let is_connected = new_state == PortState::Streaming;

                // --- Refactored Menu Item Manipulation ---

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
                        let text = if is_connected { "Change Device..." } else { "Connect Device..." };
                        item.set_text(text).unwrap();
                    }
                }
            }
        }
    }
    
    fn wait_for_connection(status_rx: &crossbeam::channel::Receiver<Event>) -> Result<(), ()> {
        loop {
            match status_rx.recv_timeout(Duration::from_secs(10)) {
                Ok(Event::SensorConnected) | Ok(Event::SensorReconnected) => return Ok(()),
                Ok(Event::FailedToConnect) | Ok(Event::FailedToReconnect) | Ok(Event::FatalError(_)) | Ok(Event::Exiting) => return Err(()),
                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => return Err(()),
                _ => continue, // Other events are ignored during initial connection phase.
            }
        }
    }

    fn process_incoming_sample(&self, sample: &Sample, route: &DeviceRoute) {
        let timestamp = sample.timestamp_end();
        for column in &sample.columns {
            let key = DataColumnId {
                port_url: self.url.clone(),
                device_route: route.clone(),
                stream_id: sample.stream.stream_id,
                column_index: column.desc.index,
            };

            if let Some(value) = column.value.try_as_f64() {
                let point = Point { x: timestamp, y: value };
                self.capture.insert(&key, point);
            }
        }
    }

    pub fn list_rpcs_for_device(device: &mut Device) -> Vec<RpcMeta> {
        let mut rpc_metas = Vec::new();
        
        let rpc_count: u16 = match device.get("rpc.listinfo") {
            Ok(count) => count,
            Err(_) => return rpc_metas, // Return empty list if device doesn't support this.
        };
        
        for rpc_id in 0..rpc_count {
            if let Ok((meta_bits, name)) = device.rpc::<u16, (u16, String)>("rpc.listinfo", rpc_id) {
                
                // Parse the meta_bits directly
                let (arg_type, size) = parse_arg_type_and_size(meta_bits);
                let permissions = parse_permissions_string(meta_bits);
                let readable = (meta_bits & 0x0100) != 0;
                let writable = (meta_bits & 0x0200) != 0;
                let persistent = (meta_bits & 0x0400) != 0;
                let unknown = meta_bits == 0;

                // Create the RpcMeta struct directly
                let meta = RpcMeta {
                    name,
                    size,
                    permissions,
                    arg_type,
                    readable,
                    writable,
                    persistent,
                    unknown,
                    value: None, // This will be populated in the next step
                };
                rpc_metas.push(meta);
            }
        }
        rpc_metas
    }
}
