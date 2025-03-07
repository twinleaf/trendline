#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(warnings)] 
use twinleaf::{data::{ColumnData, Device}, tio::{self}};
use tio::{proto::DeviceRoute, proxy, util};
use getopts::Options;

use tauri::{http::header::Values, Emitter, Listener, LogicalPosition, LogicalSize, Manager, WebviewUrl, Window};
use welch_sde::{Build, SpectralDensity};
use std::{env, thread};
use std::collections::{HashMap, HashSet};

#[derive(serde::Serialize, Clone, Debug)]
struct GraphLabel{
    col_name: Vec<String>, 
    col_desc: Vec<String>,
    col_unit: Vec<String>,
    col_stream: Vec<u8>
} 
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
        Some(if t.is_empty() { "string".to_string() } else { t })
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
            Some(if t.is_empty() { "string".to_string() } else { t })
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
                std::str::from_utf8(&reply).unwrap_or_default(),
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

fn rpc_controls(args: &[String], column_names: Vec<String>) -> Vec<(String, bool)>  {
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, args);
    let proxy = proxy::Interface::new(&root);
    let device = proxy.device_rpc(route).unwrap();

    let nrpcs: u16 = device.get("rpc.listinfo").unwrap();

    let mut rpc_controls: Vec<(String, bool)> = Vec::new();
    let mut seen_controls: HashSet<String> = HashSet::new();
    let mut control_names: HashMap<String, Vec<String>> = HashMap::new();

    for names in column_names {
        let parts: Vec<&str> = names.split('.').collect();
        if parts.len() > 2 {
            let prefix = format!("{}.{}", parts[0], parts[1]);
            control_names.entry(prefix).or_default().push(names.clone());
        } 
    }

    let mut rpc_map: HashMap<String, Vec<(String, bool)>> = HashMap::new();
    let mut button = false;
    for rpc_id in 0u16..nrpcs {
        let (meta, name): (u16, String) = device.rpc("rpc.listinfo", rpc_id).unwrap();
        let meta = RpcMeta::parse(meta);
        if meta.write && meta.read{button = true}
        else {button = false}
        let parts: Vec<&str> = name.split('.').collect();
        if parts.len() >= 3{
            let prefix = format!("{}.{}", parts[0], parts[1]);
            rpc_map.entry(prefix).or_default().push((name.clone(), button));
        }
    } 

    for prefix in control_names.keys() {
        if let Some(rpc_names) = rpc_map.get(prefix) {
            for (name, writeable) in rpc_names{
                if !seen_controls.contains(name) {
                    rpc_controls.push((name.clone(), *writeable));
                    seen_controls.insert(name.clone());
                }
            }
        }
    }
    rpc_controls
}

fn calc_fft(signals: Option<&Vec<f32>>, fs: f32 ) -> (Vec<f32>, Vec<f32>) {
    if let Some(signal) = signals{
        let mean = signal.iter().sum::<f32>() / signal.len() as f32;
        let mean_adjusted_signal: Vec<f32> = signal.iter().map(|x| x - mean).collect();

        let welch: SpectralDensity<f32> =
            SpectralDensity::<f32>::builder(&mean_adjusted_signal, fs).build();
        
        let result = std::panic::catch_unwind(|| {welch.periodogram();});
        if result.is_err(){
            return (vec![], vec![]);
        } else {
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
    match data {
        ColumnData::Int(x) => x as f32,
        ColumnData::UInt(x) => x as f32,
        ColumnData::Float(x) => x as f32,
        ColumnData::Unknown => 0.0,
    }
}

#[tauri::command]
fn stream_data(window: Window) {
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
        let mut stream_desc = GraphLabel{
            col_name: Vec::new(),
            col_desc: Vec::new(),
            col_unit: Vec::new(),
            col_stream: Vec::new()
        };
        for stream in meta.streams.values() {
            sampling_rates.insert(stream.stream.stream_id, vec![stream.segment.sampling_rate, stream.segment.decimation]);
            for col in &stream.columns {
                stream_desc.col_name.push(col.name.clone());
                stream_desc.col_desc.push(col.description.clone());
                stream_desc.col_unit.push(col.units.clone());
                stream_desc.col_stream.push(col.stream_id);
            }
        }

        //Emit graph labels
        let mut fft_sort: HashMap<String, Vec<String>> = HashMap::new();
        for names in stream_desc.col_name.clone() {
            let parts: Vec<&str> = names.split('.').collect();
            let prefix = parts[0].to_string();
            fft_sort.entry(prefix).or_default().push(names.clone());
        }
        let header = format!("{}\nSerial: {}", meta.device.name, meta.device.serial_number);
        
        let _= window.emit("graph_labels", (header, stream_desc.clone()));
        let _ = window.emit("fftgraphs", fft_sort);

        //Emitting RPC Commands
        let rpc_results = rpc_controls(&args, stream_desc.col_name.clone());
        window.emit("rpcs", rpc_results).unwrap();

        let mut stream_backlog: HashMap<u8, Vec<Vec<f32>>> = HashMap::new();

        //Emitting Stream Data
        loop{ 
            let sample = device.next();
            let mut col_pos = 0;
            let mut graph_backlog: Vec<Vec<f32>> = vec![Vec::new(); sample.columns.len()];

            for column in &sample.columns{                  
                graph_backlog[col_pos].push(match_value(column.value.clone()));
                col_pos += 1;
            }
            
            stream_backlog
                .entry(sample.stream.stream_id)
                .or_insert_with(|| vec![Vec::new(); sample.columns.len()])
                .iter_mut()
                .zip(graph_backlog.iter())
                .for_each(|(existing, new)| existing.extend(new.iter().cloned()));
    

            if let Some(sampling) = sampling_rates.get(&sample.stream.stream_id) {
                let fs = sampling[0] as f32/ sampling[1] as f32;
                let threshold = (fs/ 20.0).ceil() as usize;
                if let Some(values) = stream_backlog.get_mut(&sample.stream.stream_id){
                    if values.iter().all(|col| col.len() >= threshold){
                        let averaged_backlog: Vec<Vec<f32>> = graph_backlog
                            .iter()
                            .map(|col| {
                                let sum: f32 = col.iter().sum();
                                let avg = sum/col.len() as f32;
                                vec![avg]
                            })
                            .collect();
                        let _ = window.emit(&sample.stream.stream_id.clone().to_string(), &averaged_backlog);
                        values.iter_mut().for_each(|col| col.clear());
                    }
                };
            } 
        }
    });
}


#[tauri::command]
fn fft_data(window: Window) {
    let args: Vec<String> = env::args().collect();   
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, &args);

    let time_span: u8 = if args.len() > 2{
        match args[2].parse(){
            Ok(val) => val, 
            Err(_e) => 10
        } 
    } else{10};

    thread::spawn(move || {
        let proxy = proxy::Interface::new(&root);
        let device = proxy.device_full(route).unwrap();
        let mut device = Device::new(device);
        
        //get sampling & decimation rate, column metadata
        let meta = device.get_metadata();
        let mut sampling_rates: HashMap< u8, Vec<u32>> = HashMap::new();
        let mut sorted_columns: HashMap<String, Vec<String>> = HashMap::new();
        for stream in meta.streams.values() {
            sampling_rates.insert(stream.stream.stream_id, vec![stream.segment.sampling_rate, stream.segment.decimation]);
            for col in &stream.columns {
                let parts: Vec<&str> = col.name.split('.').collect();
                let prefix = parts[0].to_string();
                sorted_columns.entry(prefix).or_default().push(col.name.to_string());
            }
        }

        let mut fft_signals: HashMap<String, Vec<f32>> = HashMap::new();

        //Emitting FFT Data
        loop{ 
            let sample = device.next();
            let mut fft_freq: HashMap<String, Vec<f32>> = HashMap::new();
            let mut fft_power: HashMap<String, Vec<Vec<f32>>> = HashMap::new();

            if let Some(rate) = sampling_rates.get(&sample.stream.stream_id){
                let decimation_rate = rate[1];
                let fs = rate[0]/ decimation_rate;

                for column in sample.columns{               
                    fft_signals.entry(column.desc.name.clone()).or_default().push(match_value(column.value.clone()));

                    if let  Some(col_value) = fft_signals.get_mut(&column.desc.name.clone()){
                        if col_value.len() >= (fs*time_span as u32).try_into().unwrap(){
                            col_value.remove(0);
                        }
                        //Note: (resize on fft is breaking past length of fs) 
                        if col_value.len() >= (fs*time_span as u32 -1).try_into().unwrap() {
                            let (freq, power) = calc_fft(fft_signals.get(&column.desc.name.clone()), fs as f32);
                            if !freq.is_empty() && !power.is_empty() && !freq.iter().any(|&x| x.is_nan()) && !power.iter().any(|&x| x.is_nan()){
                                let parts: Vec<&str> = column.desc.name.split('.').collect();
                                let prefix = parts[0].to_string();
                                fft_power.entry(prefix.clone()).or_default().push(power.clone());
                                fft_freq.entry(prefix.clone()).or_insert_with(||freq.clone());
                            }
                        }
                    }
                }

                for (name, values) in &mut fft_power{
                    let mut spectrum_data: Vec<Vec<f32>> = Vec::new();
                    if let Some(freq_result) = fft_freq.get(name) {
                        spectrum_data.push(freq_result.to_vec());
                        for value in values{
                            spectrum_data.push(value.to_vec());
                        }

                        if let Some(columns) = sorted_columns.get(name){
                            if spectrum_data.len() == columns.len() + 1{
                                let averaged_spec_data: Vec<Vec<f32>> = spectrum_data
                                .iter()
                                .map(|col| {
                                    let chunk_size = (col.len() as f32/ (fs as f32/20.0)).ceil() as usize;
                                    col.chunks(chunk_size)
                                        .map(|chunk| chunk.iter().sum::<f32>() /chunk.len() as f32)
                                        .collect()
                                })
                                .collect();
                                let _ = window.emit(&name.clone(), spectrum_data);
                            }
                        }
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
                .title("Trendline") 
                .build()?;

            let _graph = window.add_child(
                tauri::webview::WebviewBuilder::new("stream-1", WebviewUrl::App(Default::default()))
                    .auto_resize(),
                    LogicalPosition::new(0., 0.),
                    LogicalSize::new(800., 600.),
                )?;

            //App listens for the RPC call and returns the result to the specified window
            let main_window = app.get_webview("stream-1").unwrap();
            let new_window = app.get_webview("stream-1").unwrap();
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
            stream_data, 
            fft_data])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
}
