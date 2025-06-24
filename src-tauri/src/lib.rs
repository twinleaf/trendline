// src-tauri/src/lib.rs

use twinleaf::tio::proxy;
use twinleaf::data::Sample;

#[derive(Debug)]
pub enum TwinleafPortInterface {
    FTDI,
    STM32,
    Unknown(u16, u16),
}

pub struct SerialDevice {
    url: String,
    ifc: TwinleafPortInterface,
}

#[derive(Debug)]
pub struct WorkerMessage {
    route: String,
    sample: Sample,
}

pub fn enum_serial_devices(all: bool) -> Vec<SerialDevice> {
    let mut ports: Vec<SerialDevice> = Vec::new();

    if let Ok(avail_ports) = serialport::available_ports() {
        for p in avail_ports.iter() {
            if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
                let interface = match (info.vid, info.pid) {
                    (0x0403, 0x6015) => TwinleafPortInterface::FTDI,
                    (0x0483, 0x5740) => TwinleafPortInterface::STM32,
                    (vid, pid) => {
                        if !all {
                            continue;
                        };
                        TwinleafPortInterface::Unknown(vid, pid)
                    }
                };
                #[cfg(target_os = "macos")]
                if p.port_name.starts_with("/dev/tty.") && !all {
                    continue;
                }
                ports.push(SerialDevice {
                    url: format!("serial://{}", p.port_name),
                    ifc: interface,
                });
            } // else ignore other types for now: bluetooth, pci, unknown
        }
    }
    ports
}

pub fn get_valid_twinleaf_serial_urls() -> Vec<String> {
    let devices = enum_serial_devices(false);
    let mut valid_urls = Vec::new();
    for dev in devices {
        match dev.ifc {
            TwinleafPortInterface::STM32 | TwinleafPortInterface::FTDI => {
                valid_urls.push(dev.url.clone());
            }
            _ => {}
        }
    }
    valid_urls
}

pub fn device_worker_thread(
    port: proxy::Port,
    route_str: String,
    sender: crossbeam::channel::Sender<WorkerMessage>,
) {
    println!("#[{}] Worker thread started.", route_str);
    let mut device = twinleaf::Device::new(port);
    loop {
        let sample = device.next();
        if sender.send(WorkerMessage { route: route_str.clone(), sample }).is_err() {
            break;
        }
    }
    println!("#[{}] Worker thread finished.", route_str);
}