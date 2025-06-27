use sysinfo::System;

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
#[expect(unused)]
pub struct LANDevice{
    ip: String,
    mac: String,
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

#[expect(unused)]
pub fn enum_lan_devices(all: bool) -> Vec<LANDevice> {
    let mut ips: Vec<LANDevice> = Vec::new();

    ips
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
