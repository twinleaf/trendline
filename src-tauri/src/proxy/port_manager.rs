use crate::state::capture::{CaptureState, DataColumnId, Point};
use crate::shared::{UiDevice, UiStream, DeviceMeta, StreamMeta, ColumnMeta, PortState};
use crate::util;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
    thread,
};
use tauri::menu::MenuItemKind;
use tauri::{Emitter, Manager};
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
    /// Execute a raw RPC on a specific device.
    Rpc {
        route: DeviceRoute,
        name: String,
        args: Vec<u8>,
        // For a real implementation, you'd add a `responder: oneshot::Sender<Result<Vec<u8>, RpcError>>`
        // to send the result back to the caller.
    },
    Shutdown,
}

pub struct PortManager {
    url: String,
    state: Mutex<PortState>,
    proxy: Mutex<Option<Arc<proxy::Interface>>>,
    devices: Mutex<HashMap<DeviceRoute, Device>>,
    command_tx: crossbeam::channel::Sender<PortCommand>,
    app: tauri::AppHandle,
    capture: CaptureState,
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
                                PortCommand::Rpc { route, name, args } => {
                                    println!("[{}] Received RPC for route '{}': {}", self_.url, route, name);
                                    if let Some(device) = devices.get_mut(&route) {
                                        // The raw_rpc method is blocking, which is expected here.
                                        match device.raw_rpc(&name, &args) {
                                            Ok(reply) => {
                                                // TODO: Send reply back to the caller via a oneshot channel.
                                                println!("[{}]   -> RPC success: reply has {} bytes", self_.url, reply.len());
                                            }
                                            Err(e) => {
                                                eprintln!("[{}]   -> RPC error: {:?}", self_.url, e);
                                            }
                                        }
                                    } else {
                                        eprintln!("[{}]   -> RPC error: device not found for route '{}'", self_.url, route);
                                    }
                                }
                            }
                        }

                        // Poll each device for new samples
                        for (route, device) in devices.iter_mut() {
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
            
            let mut sorted_streams_meta: Vec<_> = metadata.streams.values().collect();
            sorted_streams_meta.sort_by_key(|s| s.stream.stream_id);
            
            let ui_streams: Vec<UiStream> = sorted_streams_meta.into_iter().map(|device_stream_meta| {
                    let lib_stream_meta = &*device_stream_meta.stream;
                    
                    let mut ui_columns: Vec<ColumnMeta> = device_stream_meta.columns.iter().map(|lib_column_arc| {
                        ColumnMeta::from((**lib_column_arc).clone())
                    }).collect();
                    
                    ui_columns.sort_by_key(|c| c.index);

                    UiStream {
                        meta: StreamMeta::from(lib_stream_meta.clone()),
                        columns: ui_columns,
                    }
            }).collect();

            let ui_dev = UiDevice{
                url: self.url.clone(),
                route: route.to_string(),
                state: self.state.lock().unwrap().clone(),
                meta: DeviceMeta::from((*metadata.device).clone()),
                streams: ui_streams,
            };

            discovered_ui_devices.push(ui_dev);
            devices.insert(route, device);
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
                let point = Point { t: timestamp, y: value };
                self.capture.insert(&key, point);
            }
        }
    }
}
