#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use twinleaf::{data::{ColumnData, Device}, tio::{self}};
use tio::{proto::DeviceRoute, proxy, util};
use getopts::Options;

use tauri::{Emitter, Listener, LogicalPosition, LogicalSize, Manager, WebviewUrl, Window};
use welch_sde::{Build, SpectralDensity};
use std::{env, thread};
use std::collections::{HashMap, HashSet};
use serde::Deserialize;

struct RpcMeta {
    arg_type: String,
    size: usize,
    read: bool,
    write: bool,
    persistent: bool,
    unknown: bool,
}

impl RpcMeta {
    pub fn parse(meta: u16) -> RpcMeta {
        let size = ((meta >> 4) & 0xF) as usize;
        let atype = meta & 0xF;
        RpcMeta {
            arg_type: match atype {
                0 => match size {
                    1 => "u8",
                    2 => "u16",
                    4 => "u32",
                    8 => "u64",
                    _ => "",
                },
                1 => match size {
                    1 => "i8",
                    2 => "i16",
                    4 => "i32",
                    8 => "i64",
                    _ => "",
                },
                2 => match size {
                    4 => "f32",
                    8 => "f64",
                    _ => "",
                },
                3 => "string",
                _ => "",
            }
            .to_string(),
            size,
            read: (meta & 0x0100) != 0,
            write: (meta & 0x0200) != 0,
            persistent: (meta & 0x0400) != 0,
            unknown: meta == 0,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Config {
    fft: Vec<String>
}

fn tio_opts() -> Options {
    let mut opts = Options::new();
    opts.optopt(
        "r",
        "",
        &format!("sensor root (default {})", util::default_proxy_url()),
        "address",
    );
    opts.optopt(
        "s",
        "",
        "sensor path in the sensor tree (default /)",
        "path",
    );
    opts
}

fn tio_parseopts(opts: Options, args: &[String]) -> (getopts::Matches, String, DeviceRoute) {
    let matches = match opts.parse(args) {
        Ok(m) => m,
        Err(f) => {
            panic!("{}", f.to_string())
        }
    };
    let root = if let Some(url) = matches.opt_str("r") {
        url
    } else {
        "tcp://localhost".to_string()
    };
    let route = if let Some(path) = matches.opt_str("s") {
        DeviceRoute::from_str(&path).unwrap()
    } else {
        DeviceRoute::root()
    };
    (matches, root, route)
}

fn get_rpctype(
    name: &String,
    device: &proxy::Port,
) -> String {
    RpcMeta::parse(device.rpc("rpc.info", name).unwrap()).arg_type
}

#[tauri::command]
fn rpc(args: &[String]) -> std::io::Result<String> {
    let mut opts = tio_opts();
    opts.optopt(
        "t",
        "req-type",
        "RPC request type (one of u8/u16/u32/u64 i8/i16/i32/i64 f32/f64 string). ",
        "type",
    );
    opts.optopt(
        "T",
        "rep-type",
        "RPC reply type (one of u8/u16/u32/u64 i8/i16/i32/i64 f32/f64 string). ",
        "type",
    );
    opts.optflag("d", "", "Debug printouts.");
    let (matches, root, route) = tio_parseopts(opts, args);

    let rpc_name = if matches.free.is_empty() {
        panic!("must specify rpc name")
    } else {
        matches.free[0].clone()
    };

    let rpc_arg = if matches.free.len() > 2 {
        panic!("usage: name [arg]")
    } else if matches.free.len() == 2 {
        Some(matches.free[1].clone())
    } else {
        None
    };

    let debug = matches.opt_present("d");

    let (status_send, proxy_status) = crossbeam::channel::bounded::<proxy::Event>(100);
    let proxy = proxy::Interface::new_proxy(&root, None, Some(status_send));
    let device = proxy.device_rpc(route).unwrap();
    let mut result = "default".to_string() ;

    let req_type = if let Some(req_type) = matches.opt_str("req-type") {
        Some(req_type)
    } else if rpc_arg.is_some() {
        let t = get_rpctype(&rpc_name, &device);
        Some(if t == "" { "string".to_string() } else { t })
    } else {None};

    let reply = match device.raw_rpc(
        &rpc_name,
        &if rpc_arg.is_none() {
            vec![]
        } else {
            let s = rpc_arg.unwrap();
            match &req_type.as_ref().unwrap()[..] {
                "u8" => s.parse::<u8>().unwrap().to_le_bytes().to_vec(),
                "u16" => s.parse::<u16>().unwrap().to_le_bytes().to_vec(),
                "u32" => s.parse::<u32>().unwrap().to_le_bytes().to_vec(),
                "u64" => s.parse::<u32>().unwrap().to_le_bytes().to_vec(),
                "i8" => s.parse::<i8>().unwrap().to_le_bytes().to_vec(),
                "i16" => s.parse::<i16>().unwrap().to_le_bytes().to_vec(),
                "i32" => s.parse::<i32>().unwrap().to_le_bytes().to_vec(),
                "i64" => s.parse::<i32>().unwrap().to_le_bytes().to_vec(),
                "f32" => s.parse::<f32>().unwrap().to_le_bytes().to_vec(),
                "f64" => s.parse::<f64>().unwrap().to_le_bytes().to_vec(),
                "string" => s.as_bytes().to_vec(),
                _ => panic!("Invalid type"),
            }
        },
    ) {
        Ok(rep) => rep,
        Err(err) => {
            if debug {
                drop(proxy);
                println!("RPC failed: {:?}", err);
                for s in proxy_status.try_iter() {
                    println!("{:?}", s);
                }
            }
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "RPC failed"));
        }
    };

    if !reply.is_empty() {
        let rep_type = if let Some(rep_type) = matches.opt_str("rep-type") {
            Some(rep_type)
        } else if req_type.is_none() {
            let t = get_rpctype(&rpc_name, &device);
            Some(if t == "" { "string".to_string() } else { t })
        } else {req_type};
    
        let reply_str = match &rep_type.as_ref().unwrap()[..] {
            "u8" => u8::from_le_bytes(reply[0..1].try_into().unwrap()).to_string(),
            "u16" => u16::from_le_bytes(reply[0..2].try_into().unwrap()).to_string(),
            "u32" => u32::from_le_bytes(reply[0..4].try_into().unwrap()).to_string(),
            "u64" => u64::from_le_bytes(reply[0..8].try_into().unwrap()).to_string(),
            "i8" => i8::from_le_bytes(reply[0..1].try_into().unwrap()).to_string(),
            "i16" => i16::from_le_bytes(reply[0..2].try_into().unwrap()).to_string(),
            "i32" => i32::from_le_bytes(reply[0..4].try_into().unwrap()).to_string(),
            "i64" => i64::from_le_bytes(reply[0..8].try_into().unwrap()).to_string(),
            "f32" => f32::from_le_bytes(reply[0..4].try_into().unwrap()).to_string(),
            "f64" => f64::from_le_bytes(reply[0..8].try_into().unwrap()).to_string(),
            "string" => format!(
                "\"{}\" {:?}",
                if let Ok(s) = std::str::from_utf8(&reply) {
                    s
                } else {
                    ""
                },
                reply
            ),
            _ => panic!("Invalid type"),
        };
        result = reply_str.clone();
    }
    drop(proxy);
    for s in proxy_status.iter() {
        if debug {
            println!("{:?}", s);
        }
    }
    Ok(result)
}

fn rpc_controls(args: &[String], column_names: HashMap<u8, Vec<String>>) -> Vec<String>  {
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, args);
    let proxy = proxy::Interface::new(&root);
    let device = proxy.device_rpc(route).unwrap();

    let nrpcs: u16 = device.get("rpc.listinfo").unwrap();

    let suffixes = ["enable", "reset", "Kp", "Ki", "setpoint"];
    let mut rpc_controls: Vec<String> = Vec::new();
    let mut seen_controls: HashSet<String> = HashSet::new();
    let mut control_names = HashSet::new();

    for (_id, col_names) in &column_names {
        for meta_name in col_names {
            let parts: Vec<&str> = meta_name.split('.').collect();
        
            if parts.len() >= 2 {
                let prefix = format!("{}.{}", parts[0], parts[1]);
                for suffix in &suffixes{
                    let control_name = format!("{}.control.{}", prefix, suffix);
                    control_names.insert(control_name);
                }
            }   
        }     
    }

    for rpc_id in 0u16..nrpcs {
        let (_meta, name): (u16, String) = device.rpc("rpc.listinfo", rpc_id).unwrap();
        if control_names.contains(&name) && !seen_controls.contains(&name) {
            seen_controls.insert(name);
        }
    }     

    for suffix in &suffixes {
        for control_name in &seen_controls{
            if control_name.ends_with(suffix) {
                rpc_controls.push(control_name.clone());
            }
        }
    }   
    rpc_controls
}

fn calc_fft(signal: &Vec<f32>, sampling: Option<&Vec<u32>>) -> (Vec<f32>, Vec<f32>) {
    if let Some(sampling) = sampling {
        if signal.len() <= 500{
            let decimation_rate = sampling[1] as f32;
            let fs = sampling[0] as f32/ decimation_rate;

            let welch: SpectralDensity<f32> =
                SpectralDensity::<f32>::builder(&signal, fs).build();
            let sd = welch.periodogram();
    
            let frequencies: Vec<f32> = sd.frequency().into_iter().collect();
            let mut power_spectrum: Vec<f32> = sd.to_vec();
            
            for value in &mut power_spectrum {
                *value = value.sqrt();
            }
            return (frequencies, power_spectrum)
        }
    }
    (vec![], vec![])
}

fn match_value(data: ColumnData) -> f32 {
    let data_type = match data {
        ColumnData::Int(x) => x as f32,
        ColumnData::UInt(x) => x as f32,
        ColumnData::Float(x) => x as f32,
        ColumnData::Unknown => 0.0,
    };
    data_type
}

fn read_yaml(args: Vec<String>) -> Config {
    let default = "../src-tauri/src/sample.yaml".to_string();
    let path = args.get(2).unwrap_or(&default);
    let yaml_content = std::fs::read_to_string(path).expect("failed to read yaml");
    let results: Config = serde_yaml::from_str(&yaml_content).expect("failed to parse yaml");
    results
}

#[tauri::command]
//main tauri function where stream data gets emitted to graphs on frontend
fn graph_data(window: Window) {
    let args: Vec<String> = env::args().collect();   
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, &args);

    thread::spawn(move || {
        let proxy = proxy::Interface::new(&root);
        let device = proxy.device_full(route).unwrap();
        let mut device = Device::new(device);
        
        //get sampling & decimation rate, column metadata
        let meta = device.get_metadata();
        let mut sampling_rates: HashMap< u8, Vec<u32>> = HashMap::new();
        let mut column_names: HashMap<u8, Vec<String>> = HashMap::new();
        for (_id, stream) in &meta.streams {
            sampling_rates.insert(stream.stream.stream_id, vec![stream.segment.sampling_rate, stream.segment.decimation]);
            for col in &stream.columns {
                column_names.entry(stream.stream.stream_id).or_insert_with(||Vec::new());
                if let Some(names) = column_names.get_mut(&stream.stream.stream_id){
                    names.push(col.name.clone());
                }
            }
        }

        let stream_id: u8 = if args.len() >1 {
            match args[1].parse(){
                Ok(val) => val,
                Err(_e) => 1
            }
        } else {1};

        let rpc_results = rpc_controls(&args, column_names);
        let results = read_yaml(args);
        window.emit("rpcs", rpc_results).unwrap();

        let mut elapsed = std::time::Instant::now();
        let mut fft_signal: Vec<f32> = Vec::new();
        let mut backlog: Vec<Vec<f32>> = Vec::new();

        loop{ 
            let sample = device.next();
            let header = format!("Connected to: {}   Serial: {}   Session ID: {}    Stream: {}", sample.device.name, sample.device.serial_number, sample.device.session_id, stream_id);
            let mut names: Vec<String> = Vec::new();
            let mut values: Vec<f32> = Vec::new();
            let mut col_pos = 0;
            if sample.stream.stream_id == stream_id{
                if backlog.is_empty() {
                    backlog = vec![Vec::new(); sample.columns.len()]
                }
                for column in &sample.columns{
                    names.push(column.desc.name.clone());
                    values.push(match_value(column.value.clone()));

                    backlog[col_pos].push(match_value(column.value.clone()));
                    col_pos += 1;

                    fft_signal.extend(values.clone());
                    if fft_signal.len() > 500{
                        fft_signal.remove(0);
                    }
    
                    if elapsed.elapsed() >= std::time::Duration::from_secs(1) {
                        elapsed = std::time::Instant::now();
                        let (freq, power) = calc_fft(&fft_signal, sampling_rates.get(&sample.stream.stream_id));
                        if results.fft.contains(&column.desc.name.clone()){
                            let _ = window.emit("fftgraphs", &results.fft);
                            let _ = window.emit(&column.desc.name.clone(), (freq.clone(), power.clone()));
                        };
                    }
                }  

               let decimation_info = sampling_rates.get(&sample.stream.stream_id);
                if let Some(sampling) = decimation_info {
                    let fs = sampling[0] as f32/ sampling[1] as f32;
                    if fs >= 20.0 {
                        if backlog.iter().all(|col| col.len() >= fs as usize) {
                            let _ = window.emit("main", (&backlog, &names, &header));
                            backlog.iter_mut().for_each(|col| col.clear());
                        }
                    }
                    else {
                        let _ = window.emit("main", (&backlog, &names, &header));
                        backlog.iter_mut().for_each(|col| col.clear());
                    }
                } 
            }
        }
    });
}

fn main(){
    tauri::Builder::default()
        .setup(|app| {
            let window = tauri::WindowBuilder::new(app, "main")
                .inner_size(800., 600.)
                .title("Lily")
                .build()?;

            let _aux = window.add_child(
                tauri::webview::WebviewBuilder::new("stream-2", WebviewUrl::App(Default::default()))
                    .auto_resize(),
                    LogicalPosition::new(0., 0.),
                    LogicalSize::new(800., 600.),
                )?;

            //App listens for the RPC call and returns the result to the specified window
            let main_window = app.get_webview("stream-2").unwrap();
            let new_window = app.get_webview("stream-2").unwrap();
            app.listen("returningRPCName", move |event| {
                let mut arg: Vec<String> = vec![env::args().collect(), "rpc".to_string()];   
                let rpccall: Vec<String> = serde_json::from_str(event.payload()).unwrap();
                for command in &rpccall {
                    let _ = &arg.push(command.to_string());
                }
                if let Ok(passed) = rpc(&arg[2..]) {
                    println!("Returning {} {}", rpccall[0], passed.clone());
                    let _ = main_window.emit("returnRPC", (rpccall[0].clone(), passed.clone()));
                }
            });
            
            //on load the current rpc values are loaded into the corresponding input fields
            app.listen("onLoad", move |event| {
                let mut arg: Vec<String> = vec![env::args().collect(), "rpc".to_string()]; 
                let rpccall: String = serde_json::from_str(event.payload()).unwrap();
                let _ = &arg.push(rpccall.clone());
                if let Ok(passed) = rpc(&arg[2..]) {
                    new_window.emit("returnOnLoad", (rpccall.clone(), passed.clone())).unwrap();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            graph_data])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
}
