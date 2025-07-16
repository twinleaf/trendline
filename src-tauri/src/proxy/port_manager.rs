use crate::state::capture::{CaptureState, DataColumnId, Point};
use crate::shared::{ColumnMeta, DeviceMeta, PortState, RpcMeta, UiDevice, UiStream};
use crate::util::{self, parse_arg_type_and_size, parse_permissions_string};
use std::panic::AssertUnwindSafe;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
    thread,
    panic,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::menu::MenuItemKind;
use tauri::{Emitter, Manager, async_runtime};
use twinleaf::{
    tio::{
        proto::{DeviceRoute},
        proxy::{self, Event},
    },
    Device,
};
use crossbeam::select;

pub struct DebugCounters {
    pub polls: AtomicUsize,
    pub samples_received: AtomicUsize,
    pub points_inserted: AtomicUsize,
}

// Commands that can be sent from other parts of the application into the PortManager's thread.
#[derive(Debug)]
pub enum PortCommand {
    Shutdown,
    AttemptConnection,
}

pub struct PortManager {
    pub url: String,
    pub state: Mutex<PortState>,
    connection_retries: Mutex<u32>,
    pub proxy: Mutex<Option<Arc<proxy::Interface>>>,
    pub devices: RwLock<HashMap<DeviceRoute, Arc<Mutex<(Device, UiDevice)>>>>,
    pub command_tx: crossbeam::channel::Sender<PortCommand>,
    pub app: tauri::AppHandle,
    pub capture: CaptureState,
    pub counters: DebugCounters,
}

impl PortManager {
    pub fn new(url: String, app: tauri::AppHandle, capture: CaptureState) -> Arc<Self> {
        let (command_tx, command_rx) = crossbeam::channel::unbounded();

        let pm = Arc::new(Self {
            url,
            state: Mutex::new(PortState::Idle),
            connection_retries: Mutex::new(0),
            proxy: Mutex::new(None),
            devices: RwLock::new(HashMap::new()),
            command_tx,
            app,
            capture,
            counters: DebugCounters {
                polls: AtomicUsize::new(0),
                samples_received: AtomicUsize::new(0),
                points_inserted: AtomicUsize::new(0),
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

    fn spawn_thread(
        self_: Arc<Self>,
        command_rx: crossbeam::channel::Receiver<PortCommand>,
    ) {
        thread::Builder::new()
            .name(format!("port-{}", self_.url))
            .spawn(move || {
                let (status_tx, status_rx) = crossbeam::channel::unbounded();
                let ticker = crossbeam::channel::tick(Duration::from_micros(100));

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
                                self_.set_state(PortState::Discovery);

                                if let Some(proxy_if) = self_.proxy.lock().unwrap().clone() {
                                    if self_.discover_devices(&proxy_if).is_ok() {
                                        println!("[{}] Discovery finished, beginning to stream data.", self_.url);
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
                                let polls = self_.counters.polls.swap(0, Ordering::Relaxed);
                                let points = self_.counters.points_inserted.swap(0, Ordering::Relaxed);
                                let current_state = self_.state.lock().unwrap().clone();
                                println!(
                                    "[{}] Heartbeat (30s): State={:?}, Polls={}, PointsIns={}",
                                    self_.url, current_state, polls, points
                                );
                                last_debug_print = Instant::now();
                            }
                        }
                    }
                }

                // --- Final Shutdown Cleanup ---
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
        let discovery_port = proxy_if.tree_probe()?;
        let mut discovered_routes = HashSet::new();
        let discovery_deadline = Instant::now() + Duration::from_secs(2);

        self.set_state(PortState::Discovery);
        println!("[{}] Listening for device routes...", self.url);

        while Instant::now() < discovery_deadline {
            if let Ok(pkt) = discovery_port.receiver().recv_timeout(Duration::from_millis(100)) {
                if !self.devices.read().unwrap().contains_key(&pkt.routing) {
                    discovered_routes.insert(pkt.routing);
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
                    eprintln!("[{}] Failed to build UI for device on route '{}': {:?}", self.url, route, e);
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
            
            discovered_ui_devices_for_event.push(ui_dev.clone());
            devices.insert(route, Arc::new(Mutex::new((data_device, ui_dev))));
        }

        if !discovered_ui_devices_for_event.is_empty() {
            println!("[{}] Publishing batch of {} devices", self.url, discovered_ui_devices_for_event.len());
            self.app.emit("port-devices-discovered", discovered_ui_devices_for_event).unwrap();
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
        
        println!("[{}]   -> Fetched {} streams and {} RPCs for '{}'", self.url, streams.len(), rpcs.len(), route);

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
            if let Ok((meta_bits, name)) = rpc_device.rpc::<u16, (u16, String)>("rpc.listinfo", rpc_id) {
                let (arg_type, size) = parse_arg_type_and_size(meta_bits);
                let mut meta = RpcMeta {
                    name: name.clone(),
                    size,
                    permissions: parse_permissions_string(meta_bits),
                    arg_type,
                    readable: (meta_bits & 0x0100) != 0,
                    writable: (meta_bits & 0x0200) != 0,
                    persistent: (meta_bits & 0x0400) != 0,
                    unknown: meta_bits == 0,
                    value: None,
                };

                if meta.readable {
                    let result = panic::catch_unwind(AssertUnwindSafe(|| rpc_device.raw_rpc(&name, &[])));
                    if let Ok(Ok(reply_bytes)) = result {
                        meta.value = util::bytes_to_json_value(&reply_bytes, &meta.arg_type);
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
        
        let ui_streams: Vec<UiStream> = sorted_streams.into_iter().map(|s| {
            let mut ui_columns: Vec<ColumnMeta> = s.columns.iter().map(|c| ColumnMeta::from((**c).clone())).collect();
            ui_columns.sort_by_key(|c| c.index);

            let segment_data = (*s.segment).clone();

            let decimation = if segment_data.decimation > 0 { segment_data.decimation as f64 } else { 1.0 };
            let effective_sampling_rate = segment_data.sampling_rate as f64 / decimation;
            
            UiStream {
                meta: (*s.stream).clone().into(),
                segment: Some(segment_data.into()),
                columns: ui_columns,
                effective_sampling_rate,
            }
        }).collect();

        (device_meta, ui_streams)
    }
    
    fn set_state(&self, new_state: PortState) {
        *self.state.lock().unwrap() = new_state.clone();
        self.app
            .emit("port-state-changed", (self.url.clone(), new_state.clone()))
            .unwrap();


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
                        let text = if is_connected { "Change Device..." } else { "Connect Device..." };
                        item.set_text(text).unwrap();
                    }
                }
            }
        }
    }

    fn poll_device_data(self: &Arc<Self>) {
        self.counters.polls.fetch_add(1, Ordering::Relaxed);
        
        let mut refresh_needed = HashSet::new();
        let mut reinit_needed = HashSet::new();

        // {
        //     let devices_map = self.devices.read().unwrap();
        //     for (route, entry) in devices_map.iter() {
        //         let mut tuple = entry.lock().unwrap();
        //         let (device, _) = &mut *tuple;
        //         if let Ok(samples) = device.drain() {
        //             for sample in samples {
        //                 self.process_sample_data(route, &sample);
        //                 if sample.meta_changed || sample.segment_changed {
        //                     refresh_needed.insert(route.clone());
        //                 }
        //             }
        //         } else {
        //             eprintln!("[{}] Error draining device '{}'. Connection may be lost.", self.url, route);
        //             // HELP
        //             return;
        //         }
        //     }
        // }
        {
            let devices_map = self.devices.read().unwrap();
            for (route, entry) in devices_map.iter() {
                let mut tuple = entry.lock().unwrap();
                let (device, _) = &mut *tuple;

                // Use catch_unwind to handle the potential panic in drain()
                let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    device.drain()
                }));

                match result {
                    Ok(samples) => {
                        for sample in samples {
                            self.process_sample_data(route, &sample);
                            if sample.meta_changed || sample.segment_changed {
                                refresh_needed.insert(route.clone());
                            }
                        }
                    },
                    Err(_) => {
                        println!("[{}] Device '{}' unresponsive. Attempting to reinstate handler.", self.url, route);
                        reinit_needed.insert(route.clone());
                    }
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
                        println!("[{}] Re-initialized TIO device for route '{}'.", self.url, route);
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
                
                // Use spawn_blocking for synchronous, blocking I/O
                async_runtime::spawn_blocking(move || {
                    println!("[{}] Spawning background task to refresh metadata for route '{}'", self_clone.url, route);

                    let mut device_tuple = device_arc_clone.lock().unwrap();
                    let (data_device, ui_device) = &mut *device_tuple;
                    
                    let (new_meta, new_streams) = self_clone.fetch_metadata(data_device);

                    for stream in &new_streams {
                        let key = DataColumnId {
                            port_url: self_clone.url.clone(),
                            device_route: route.clone(),
                            stream_id: stream.meta.stream_id,
                            column_index: 0,
                        };
                        self_clone.capture.update_effective_sampling_rate(&key, stream.effective_sampling_rate);
                    }
                    ui_device.meta = new_meta;
                    ui_device.streams = new_streams;
                    
                    self_clone.app.emit("device-metadata-updated", ui_device.clone()).unwrap();
                });
            }
        }
    }

    // fn process_sample_data(&self, route: &DeviceRoute, sample: &twinleaf::data::Sample) {
    //     for column in &sample.columns {
    //         if let Some(value) = column.value.try_as_f64() {
    //             let key = DataColumnId {
    //                 port_url: self.url.clone(),
    //                 device_route: route.clone(),
    //                 stream_id: sample.stream.stream_id,
    //                 column_index: column.desc.index,
    //             };
    //             let point = Point { x: sample.timestamp_end(), y: value };
    //             self.capture.insert(&key, point);
    //             self.counters.points_inserted.fetch_add(1, Ordering::Relaxed);
    //         }
    //     }
    // }
    fn process_sample_data(&self, route: &DeviceRoute, sample: &twinleaf::data::Sample) {
        for column in &sample.columns {
            // Inlined logic from try_as_f64
            let value_f64 = match column.value {
                twinleaf::data::ColumnData::Int(i) => Some(i as f64),
                twinleaf::data::ColumnData::UInt(u) => Some(u as f64),
                twinleaf::data::ColumnData::Float(f) => Some(f),
                _ => None, // Handles Unknown or any other variants
            };

            if let Some(value) = value_f64 {
                let key = DataColumnId {
                    port_url: self.url.clone(),
                    device_route: route.clone(),
                    stream_id: sample.stream.stream_id,
                    column_index: column.desc.index,
                };
                let point = Point { x: sample.timestamp_end(), y: value };
                self.capture.insert(&key, point);
                self.counters.points_inserted.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
