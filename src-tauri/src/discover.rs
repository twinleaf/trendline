// In a file like src-tauri/src/discover.rs or a new src-tauri/src/view_models.rs

use serde::Serialize;
use twinleaf::data::{DeviceFullMetadata};
use twinleaf::tio::proxy;
use crossbeam::channel::Sender;
use sysinfo::{System};
use ts_rs::TS;

pub struct MetadataWorkerMessage {
    pub route: String,
    pub url: String,
    pub metadata: DeviceFullMetadata,
}

#[derive(serde::Deserialize, Debug, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct PortInfo {
    pub id: String,
    pub url: String,
}

// A ViewModel for a single data column in a stream
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct FeColumnInfo {
    pub name: String,
    pub units: String,
    pub description: String,
}

// A ViewModel for a single data stream
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct FeStreamInfo {
    pub id: u8,
    pub name: String,
    pub columns: Vec<FeColumnInfo>,
}

// The main ViewModel for a device, designed for the frontend tree view.
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct FeDeviceMeta {
    pub name: String,
    pub serial: String,
    pub firmware: String,

    pub route: String, 
    pub url: String,

    pub streams: Vec<FeStreamInfo>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<FeDeviceMeta>>,
}


pub fn translate_to_frontend_meta(
    full_meta: &DeviceFullMetadata,
    route: String,
    url: String,
    children: Option<Vec<FeDeviceMeta>>,
) -> FeDeviceMeta {
    let streams: Vec<FeStreamInfo> = full_meta
        .streams
        .values()
        .map(|stream_meta| {
            let columns = stream_meta
                .columns
                .iter()
                .map(|col_meta| FeColumnInfo {
                    name: col_meta.name.clone(),
                    units: col_meta.units.clone(),
                    description: col_meta.description.clone(),
                })
                .collect();

            FeStreamInfo {
                id: stream_meta.stream.stream_id,
                name: stream_meta.stream.name.clone(),
                columns,
            }
        })
        .collect();

    FeDeviceMeta {
        name: full_meta.device.name.clone(),
        serial: full_meta.device.serial_number.clone(),
        firmware: full_meta.device.firmware_hash.clone(),
        route,
        url,
        streams,
        children,
    }
}


pub fn metadata_worker_thread(
    port: proxy::Port,
    route_str: String,
    url: String,
    sender: Sender<MetadataWorkerMessage>,
) {
    println!("#[{}] Metadata worker started.", route_str);
    let mut device = twinleaf::Device::new(port);

    let metadata = device.get_metadata();
    let device_name = metadata.device.name.clone();

    let msg = MetadataWorkerMessage {
        route: route_str,
        url,
        metadata,
    };

    if sender.send(msg).is_err() {
        eprintln!("#[{}] Failed to send metadata back to main thread.", device_name);
    }
    println!("#[{}] Metadata worker finished.", device_name);
}

pub fn is_process_running(exe_name: &str) -> bool {
    let mut sys = System::new_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for process in sys.processes().values() {
        if let Some(path) = process.exe() {
            if path.file_stem().and_then(|s| s.to_str()) == Some(exe_name) {
                return true;
            }
        }
    }
    false
}