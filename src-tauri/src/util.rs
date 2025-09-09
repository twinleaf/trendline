use serde::{self, de::Error, Deserialize, Deserializer, Serializer};
use serde_json::{json, Value};
use sysinfo::System;
use tauri::menu::{Menu, MenuItemKind, Submenu};
use tauri::Runtime;
use twinleaf::tio::proto::DeviceRoute;
use twinleaf::tio::util::TioRpcReplyable;

use crate::shared::{PlotData, Point, StatisticSet};

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
pub struct LANDevice {
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

pub fn find_submenu_by_text<'a, R: Runtime>(
    root_menu: &'a Menu<R>,
    submenu_text: &str,
) -> Option<Submenu<R>> {
    if let Ok(items) = root_menu.items() {
        for item in items {
            if let MenuItemKind::Submenu(submenu) = item {
                if submenu.text().ok().as_deref() == Some(submenu_text) {
                    return Some(submenu);
                }
            }
        }
    }
    None
}

pub fn serialize<S>(route: &DeviceRoute, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&route.to_string())
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<DeviceRoute, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    DeviceRoute::from_str(&s)
        .map_err(|_err| Error::custom(format!("Invalid DeviceRoute String: '{}'", s)))
}

pub fn bytes_to_json_value(reply_bytes: &[u8], rpc_type: &str) -> Option<Value> {
    if reply_bytes.is_empty() {
        return Some(json!(null));
    }

    match rpc_type {
        "u8" => u8::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "u16" => u16::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "u32" => u32::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "u64" => u64::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "i8" => i8::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "i16" => i16::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "i32" => i32::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "i64" => i64::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "f32" => f32::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "f64" => f64::from_reply(reply_bytes).ok().map(|v| json!(v)),
        "string" => Some(json!(String::from_utf8_lossy(reply_bytes))),
        _ => {
            if rpc_type.starts_with("string<") {
                Some(json!(String::from_utf8_lossy(reply_bytes)))
            } else {
                eprintln!(
                    "[Warning]: Unhandled non-empty reply for RPC type '{}'",
                    rpc_type
                );
                None
            }
        }
    }
}

pub fn json_to_bytes(args: Option<Value>, rpc_type: &str) -> Result<Vec<u8>, String> {
    let args = match args {
        None => return Ok(vec![]),
        Some(val) => val,
    };

    match rpc_type.split('<').next().unwrap_or(rpc_type) {
        "string" => {
            if let Some(s) = args.as_str() {
                Ok(s.as_bytes().to_vec())
            } else {
                Err(format!("Expected a string for RPC, but got: {:?}", args))
            }
        }
        "u8" => args
            .as_u64()
            .map(|v| (v as u8).to_le_bytes().to_vec())
            .ok_or_else(|| "Expected a u8".into()),
        "u16" => args
            .as_u64()
            .map(|v| (v as u16).to_le_bytes().to_vec())
            .ok_or_else(|| "Expected a u16".into()),
        "u32" => args
            .as_u64()
            .map(|v| (v as u32).to_le_bytes().to_vec())
            .ok_or_else(|| "Expected a u32".into()),
        "u64" => args
            .as_u64()
            .map(|v| v.to_le_bytes().to_vec())
            .ok_or_else(|| "Expected a u64".into()),

        "i8" => args
            .as_i64()
            .map(|v| (v as i8).to_le_bytes().to_vec())
            .ok_or_else(|| "Expected an i8".into()),
        "i16" => args
            .as_i64()
            .map(|v| (v as i16).to_le_bytes().to_vec())
            .ok_or_else(|| "Expected an i16".into()),
        "i32" => args
            .as_i64()
            .map(|v| (v as i32).to_le_bytes().to_vec())
            .ok_or_else(|| "Expected an i32".into()),
        "i64" => args
            .as_i64()
            .map(|v| v.to_le_bytes().to_vec())
            .ok_or_else(|| "Expected an i64".into()),

        "f32" => args
            .as_f64()
            .map(|v| (v as f32).to_le_bytes().to_vec())
            .ok_or_else(|| "Expected an f32".into()),
        "f64" => args
            .as_f64()
            .map(|v| v.to_le_bytes().to_vec())
            .ok_or_else(|| "Expected an f64".into()),

        _ => Err(format!("Unsupported RPC argument type: {}", rpc_type)),
    }
}

pub fn parse_arg_type_and_size(meta_bits: u16) -> (String, usize) {
    let size_code = ((meta_bits >> 4) & 0xF) as usize;
    let type_code = meta_bits & 0xF;

    let type_str = match type_code {
        0 => match size_code {
            1 => "u8",
            2 => "u16",
            4 => "u32",
            8 => "u64",
            _ => "",
        },
        1 => match size_code {
            1 => "i8",
            2 => "i16",
            4 => "i32",
            8 => "i64",
            _ => "",
        },
        2 => match size_code {
            4 => "f32",
            8 => "f64",
            _ => "",
        },
        3 => "string",
        _ => "",
    }
    .to_string();

    let final_type_str = if type_str == "string" && size_code != 0 {
        format!("string<{}>", size_code)
    } else {
        type_str
    };

    (final_type_str, size_code)
}

pub fn parse_permissions_string(meta_bits: u16) -> String {
    if meta_bits == 0 {
        // is_unknown
        return "???".to_string();
    }
    format!(
        "{}{}{}",
        if (meta_bits & 0x0100) != 0 { "R" } else { "-" }, // readable
        if (meta_bits & 0x0200) != 0 { "W" } else { "-" }, // writable
        if (meta_bits & 0x0400) != 0 { "P" } else { "-" }  // persistent
    )
}

pub fn calculate_batch_stats(points: &[Point]) -> StatisticSet {
    if points.is_empty() {
        return StatisticSet::default();
    }

    let mut finite_vals: Vec<f64> = Vec::with_capacity(points.len());
    let mut nan_count: u64 = 0;

    for p in points {
        if p.y.is_finite() {
            finite_vals.push(p.y);
        } else {
            nan_count += 1;
        }
    }

    if finite_vals.is_empty() {
        return StatisticSet {
            count: 0,
            nan_count,
            mean: 0.0,
            min: 0.0,
            max: 0.0,
            stdev: 0.0,
            rms: 0.0,
        };
    }

    let count = finite_vals.len() as u64;

    let sum: f64 = finite_vals.iter().sum();
    let mean = sum / count as f64;

    let mut min = finite_vals[0];
    let mut max = finite_vals[0];
    for &v in &finite_vals {
        if v < min { min = v; }
        if v > max { max = v; }
    }

    let variance = if count > 1 {
        finite_vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / ((count - 1) as f64)
    } else {
        0.0
    };
    let stdev = variance.sqrt();

    let sum_sq: f64 = finite_vals.iter().map(|v| v.powi(2)).sum();
    let rms = (sum_sq / count as f64).sqrt();

    StatisticSet {
        count,
        nan_count,
        mean,
        min,
        max,
        stdev,
        rms,
    }
}

pub fn lerp(p1: &Point, p2: &Point, x: f64) -> f64 {
    if (p2.x - p1.x).abs() < 1e-9 {
        return p1.y;
    }
    p1.y + (p2.y - p1.y) * (x - p1.x) / (p2.x - p1.x)
}

pub fn k_way_merge_plot_data(all_series_data: Vec<PlotData>) -> PlotData {
    if all_series_data.is_empty() {
        return PlotData::empty();
    }

    let continuous_series: Vec<Vec<Point>> = all_series_data
        .iter()
        .filter_map(|plot_data| {
            if !plot_data.timestamps.is_empty() && plot_data.series_data.len() == 1 {
                Some(
                    plot_data
                        .timestamps
                        .iter()
                        .zip(plot_data.series_data[0].iter())
                        .map(|(&x, &y)| Point { x, y })
                        .collect(),
                )
            } else {
                None
            }
        })
        .collect();

    let mut series_iters: Vec<_> = continuous_series
        .iter()
        .map(|s| s.iter().peekable())
        .collect();
    if series_iters.is_empty() {
        return PlotData::empty();
    }
    let num_series = series_iters.len();
    let mut merged_plot_data = PlotData::with_series_capacity(num_series);
    let mut last_points: Vec<Option<Point>> = vec![None; num_series];

    loop {
        let next_ts = series_iters
            .iter_mut()
            .filter_map(|it| it.peek().map(|p| p.x))
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if let Some(ts) = next_ts {
            merged_plot_data.timestamps.push(ts);
            for i in 0..num_series {
                let iter = &mut series_iters[i];
                let mut y_val = f64::NAN; // Use NaN for gaps
                if iter.peek().map_or(false, |p| (p.x - ts).abs() < 1e-9) {
                    let p_next = iter.next().unwrap();
                    last_points[i] = Some(*p_next);
                    y_val = p_next.y;
                } else if let (Some(p_next_real), Some(p_last)) = (iter.peek(), last_points[i]) {
                    y_val = lerp(&p_last, p_next_real, ts);
                }
                merged_plot_data.series_data[i].push(y_val);
            }
        } else {
            break; // No more points in any iterator
        }
    }
    merged_plot_data
}
