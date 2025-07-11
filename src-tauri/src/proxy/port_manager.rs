use crate::state::capture::{CaptureState, DataColumnId, Point};
use crate::shared::{ColumnMeta, DeviceMeta, PortState, RpcMeta, UiDevice, UiStream};
use crate::util::{self, parse_arg_type_and_size, parse_permissions_string};
use std::panic::AssertUnwindSafe;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
    thread,
    panic,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::menu::MenuItemKind;
use tauri::{Emitter, Manager};
use twinleaf::{
    tio::{
        proto::{DeviceRoute},
        proxy::{self, Event},
    },
    Device,
};

pub struct DebugCounters {
    pub polls: AtomicUsize,
    pub samples_received: AtomicUsize,
    pub points_inserted: AtomicUsize,
}

// Commands that can be sent from other parts of the application into the PortManager's thread.
#[derive(Debug)]
pub enum PortCommand {
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
    pub counters: DebugCounters,
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
            counters: DebugCounters {
                polls: AtomicUsize::new(0),
                samples_received: AtomicUsize::new(0),
                points_inserted: AtomicUsize::new(0),
            },
        });

        Self::spawn_thread(pm.clone(), command_rx);
        pm
    }

    fn spawn_thread(
        self_: Arc<Self>,
        command_rx: crossbeam::channel::Receiver<PortCommand>,
    ) {
        thread::Builder::new()
            .name(format!("port-{}", self_.url))
            .spawn(move || {
                const MAX_RETRIES: u32 = 3;
                let mut consecutive_failures = 0;

                'lifecycle: loop {
                    if consecutive_failures >= MAX_RETRIES {
                        let err_msg = format!("Connection failed after {} attempts. Giving up.", MAX_RETRIES);
                        eprintln!("[{}] {}", self_.url, err_msg);
                        self_.set_state(PortState::Error(err_msg));
                        break 'lifecycle;
                    }
                    self_.set_state(PortState::Connecting);
                    println!("[{}] Establishing proxy...", self_.url);

                    let (status_tx, status_rx) = crossbeam::channel::unbounded();
                    let reconnect_timeout = Some(Duration::from_secs(5));

                    let proxy_if = Arc::new(proxy::Interface::new_proxy(
                        &self_.url,
                        reconnect_timeout,
                        Some(status_tx.clone()),
                    ));
                    *self_.proxy.lock().unwrap() = Some(proxy_if.clone());

                    match Self::wait_for_connection(&status_rx) {
                        Ok(_) => {
                            self_.set_state(PortState::Connected);
                            println!("[{}] Proxy connected.", self_.url);
                        },
                        Err(_) => {
                            println!("[{}] Failed to connect. Will retry.", self_.url);
                            self_.set_state(PortState::Disconnected);
                            *self_.proxy.lock().unwrap() = None;
                            consecutive_failures += 1;
                            thread::sleep(Duration::from_secs(5));
                            continue 'lifecycle;
                        }
                    }

                    if let Err(e) = self_.discover_devices(&proxy_if) {
                        eprintln!("[{}] Discovery failed: {:?}. Retrying.", self_.url, e);
                        consecutive_failures += 1;
                        thread::sleep(Duration::from_secs(5)); 
                        continue 'lifecycle;
                    }

                    consecutive_failures = 0;
                    println!("[{}] Discovery finished. Entering streaming mode.", self_.url);
                    self_.set_state(PortState::Streaming);
                    
                    let mut last_debug_print = Instant::now();

                    loop {
                        if let Ok(event) = status_rx.try_recv() {
                           match event {
                                Event::SensorDisconnected | Event::Exiting | Event::FatalError(_) => {
                                    println!("[{}] Proxy-level disconnect event received: {:?}. Restarting connection.", self_.url, event);
                                    self_.set_state(PortState::Reconnecting);
                                    continue 'lifecycle;
                                }
                                _ => {} 
                           }
                        }

                        if let Ok(command) = command_rx.try_recv() {
                           match command {
                                PortCommand::Shutdown => {
                                    println!("[{}] Shutdown command received.", self_.url);
                                    break 'lifecycle;
                                }
                           }
                        }

                        self_.poll_device_data();

                        if last_debug_print.elapsed() > Duration::from_secs(30) {
                            let polls = self_.counters.polls.swap(0, Ordering::Relaxed);
                            let samples = self_.counters.samples_received.swap(0, Ordering::Relaxed);
                            let points = self_.counters.points_inserted.swap(0, Ordering::Relaxed);
                            let current_state = self_.state.lock().unwrap().clone();
                            println!(
                                "[{}] Heartbeat (30s): State={:?}, Polls={}, SamplesRx={}, PointsIns={}",
                                self_.url, current_state, polls, samples, points
                            );
                            last_debug_print = Instant::now();
                        }

                        thread::sleep(Duration::from_millis(1));
                    }
                }
                
                // --- SHUTDOWN ---
                println!("[{}] Port manager thread finished.", self_.url);
                self_.devices.lock().unwrap().clear();
                *self_.proxy.lock().unwrap() = None;
                if *self_.state.lock().unwrap() != PortState::Error("".to_string()) {
                    self_.set_state(PortState::Disconnected);
                }
            })
            .expect("Failed to spawn port manager thread");
    }


    fn wait_for_connection(status_rx: &crossbeam::channel::Receiver<Event>) -> Result<(), ()> {
        loop {
            match status_rx.recv_timeout(Duration::from_secs(10)) {
                Ok(Event::SensorConnected) | Ok(Event::SensorReconnected) => return Ok(()),
                Ok(Event::FailedToConnect) | Ok(Event::FailedToReconnect) | Ok(Event::FatalError(_)) | Ok(Event::Exiting) => return Err(()),
                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => return Err(()),
                _ => continue,
            }
        }
    }
    
    fn discover_devices(&self, proxy_if: &Arc<proxy::Interface>) -> Result<(), proxy::PortError> {
        let discovery_port = proxy_if.tree_probe()?;
        let mut discovered_routes = HashSet::new();
        let discovery_deadline = Instant::now() + Duration::from_secs(2);

        self.set_state(PortState::Discovery);
        println!("[{}] Starting device discovery...", self.url);
        println!("[{}] Listening for device routes...", self.url);

        while Instant::now() < discovery_deadline {
            if let Ok(pkt) = discovery_port.receiver().recv_timeout(Duration::from_millis(100)) {
                if !self.devices.lock().unwrap().contains_key(&pkt.routing) {
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


        let mut devices = self.devices.lock().unwrap();
        let mut discovered_ui_devices_for_event = Vec::new();
        for (route, ui_dev) in discovered_info {
            let data_device = Device::open(proxy_if, route.clone());
            
            discovered_ui_devices_for_event.push(ui_dev.clone());
            devices.insert(route, (data_device, ui_dev));
        }

        if !discovered_ui_devices_for_event.is_empty() {
            println!("[{}] -> Publishing batch of {} devices", self.url, discovered_ui_devices_for_event.len());
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
        let metadata = data_device.get_metadata();
        
        let device_meta = DeviceMeta::from((*metadata.device).clone());

        let mut sorted_streams: Vec<_> = metadata.streams.values().collect();
        sorted_streams.sort_by_key(|s| s.stream.stream_id);
        
        let ui_streams: Vec<UiStream> = sorted_streams.into_iter().map(|s| {
            let mut ui_columns: Vec<ColumnMeta> = s.columns.iter().map(|c| ColumnMeta::from((**c).clone())).collect();
            ui_columns.sort_by_key(|c| c.index);
            UiStream {
                meta: (*s.stream).clone().into(),
                segment: Some((*s.segment).clone().into()),
                columns: ui_columns,
            }
        }).collect();

        (device_meta, ui_streams)
    }

    fn update_capture_state(&self, route: &DeviceRoute, streams: &[UiStream]) {
    for stream in streams {
        if let Some(segment) = &stream.segment {
            let key = DataColumnId {
                port_url: self.url.clone(),
                device_route: route.clone(),
                stream_id: stream.meta.stream_id,
                column_index: 0,
            };
            let sampling_rate = segment.sampling_rate as f64;
            let decimation = if segment.decimation > 0 { segment.decimation as f64 } else { 1.0 };
            self.capture.update_effective_sampling_rate(&key, sampling_rate / decimation);
        }
    }
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

    fn poll_device_data(&self) {
        self.counters.polls.fetch_add(1, Ordering::Relaxed);
        
        let mut refresh_needed = HashSet::new();
        
        let mut devices_lock = self.devices.lock().unwrap();
        for (route, (device, _)) in devices_lock.iter_mut() {
            match device.drain() {
                Ok(samples) => {
                    for sample in samples {
                        self.process_sample_data(route, &sample);

                        if sample.meta_changed || sample.segment_changed {
                            refresh_needed.insert(route.clone());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error draining device at route {:?}: {:?}", route, e);
                }
            }
        }
        drop(devices_lock); 

        if refresh_needed.is_empty() {
            return;
        }

        let mut devices_lock = self.devices.lock().unwrap();
        for route in refresh_needed {
            if let Some((data_device, ui_device)) = devices_lock.get_mut(&route) {
                let (new_meta, new_streams) = self.fetch_metadata(data_device);
                
                self.update_capture_state(&route, &new_streams);
                
                ui_device.meta = new_meta;
                ui_device.streams = new_streams;
                
                self.app.emit("device-metadata-updated", ui_device.clone()).unwrap();
            }
        }
    }

    fn process_sample_data(&self, route: &DeviceRoute, sample: &twinleaf::data::Sample) {
        for column in &sample.columns {
            if let Some(value) = column.value.try_as_f64() {
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
