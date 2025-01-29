#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use twinleaf::{data::{ColumnData, Device}, tio::{self}};
use tio::{proto::DeviceRoute, proxy, util};
use getopts::Options;

use tauri::{Emitter, Listener, LogicalPosition, LogicalSize, Manager, WebviewUrl, Window};
use welch_sde::{Build, SpectralDensity};
use ratelimit::Ratelimiter;
use std::{env, thread, time::Duration};
use std::collections::{HashMap, HashSet};

#[derive(serde::Serialize, Clone)]
struct GraphLabel{
    col_name: Vec<String>, 
    col_desc: Vec<String>
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

fn rpc_controls(args: &[String], column_names: Vec<String>) -> Vec<String>  {
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, args);
    let proxy = proxy::Interface::new(&root);
    let device = proxy.device_rpc(route).unwrap();

    let nrpcs: u16 = device.get("rpc.listinfo").unwrap();

    let mut rpc_controls: Vec<String> = Vec::new();
    let mut seen_controls: HashSet<String> = HashSet::new();
    let mut control_names: HashMap<String, Vec<String>> = HashMap::new();

    for names in column_names {
        let parts: Vec<&str> = names.split('.').collect();
        if parts.len() > 2 {
            let prefix = format!("{}.{}", parts[0], parts[1]);
            control_names.entry(prefix).or_default().push(names.clone());
        }
    }

    let mut rpc_map: HashMap<String, Vec<String>> = HashMap::new();
    for rpc_id in 0u16..nrpcs {
        let (_meta, name): (u16, String) = device.rpc("rpc.listinfo", rpc_id).unwrap();
        let parts: Vec<&str> = name.split('.').collect();
        if parts.len() >= 3{
            let prefix = format!("{}.{}", parts[0], parts[1]);
            rpc_map.entry(prefix).or_default().push(name.clone());
        }
    } 

    for (prefix, _names) in control_names {
        if let Some(rpc_names) = rpc_map.get(&prefix) {
            for name in rpc_names{
                if !seen_controls.contains(name) {
                    rpc_controls.push(name.clone());
                    seen_controls.insert(name.clone());
                }
            }
        }
    }
    rpc_controls
}

fn calc_fft(signals: Option<&Vec<f32>>, sampling: Option<&Vec<u32>>) -> (Vec<f32>, Vec<f32>) {
    if let Some(sampling) = sampling {
        if let Some(signal) = signals{
            if signal.len() <= 500{
                let decimation_rate = sampling[1] as f32;
                let fs = sampling[0] as f32/ decimation_rate;
                let welch: SpectralDensity<f32> =
                    SpectralDensity::<f32>::builder(signal, fs).build();
                
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

#[tauri::command]
fn stream_data(window: Window) {
    let args: Vec<String> = env::args().collect();   
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, &args);

    let stream_id: u8 = if args.len() >1 {
        match args[1].parse(){
            Ok(val) => val,
            Err(_e) => 1
        }
    } else {1};

    thread::spawn(move || {
        let proxy = proxy::Interface::new(&root);
        let device = proxy.device_full(route).unwrap();
        let mut device = Device::new(device);
        
        //get sampling & decimation rate, column metadata
        let meta = device.get_metadata();
        let mut sampling_rates: HashMap< u8, Vec<u32>> = HashMap::new();
        let mut stream_desc = GraphLabel{
            col_name: Vec::new(),
            col_desc: Vec::new()
        };
        for stream in meta.streams.values() {
            sampling_rates.insert(stream.stream.stream_id, vec![stream.segment.sampling_rate, stream.segment.decimation]);
            for col in &stream.columns {
                if stream.stream.stream_id == stream_id {
                    stream_desc.col_name.push(col.name.clone());
                    stream_desc.col_desc.push(col.description.clone());
                }
            }
        }
        //Note: Found that without a sleep the javascript does not load in emit properly
        thread::sleep(Duration::from_secs(1));
        //Emit graph labels
        let mut fft_sort: HashMap<String, Vec<String>> = HashMap::new();
        for names in stream_desc.col_name.clone() {
            let parts: Vec<&str> = names.split('.').collect();
            let prefix = parts[0].to_string();
            fft_sort.entry(prefix).or_default().push(names.clone());
        }
        let header = format!("{}\nSerial: {}\nStream: {}", meta.device.name, meta.device.serial_number, stream_id);
        let _= window.emit("graph_labels", (header, stream_desc.clone()));

        //Emit FFT Graph setup
        let mut key_names: Vec<String> = Vec::new();
        for key in fft_sort.keys(){
            key_names.push(key.to_string())
        }
        let _ = window.emit("fftgraphs", (key_names, fft_sort));

        //Emitting RPC Commands
        let rpc_results = rpc_controls(&args, stream_desc.col_name.clone());
        window.emit("rpcs", rpc_results).unwrap();

        let mut graph_backlog: Vec<Vec<f32>> = Vec::new();

        //Emitting Stream Data
        loop{ 
            let sample = device.next();
            let mut col_pos = 0;

            if sample.stream.stream_id == stream_id{
                if graph_backlog.is_empty() {
                    graph_backlog = vec![Vec::new(); sample.columns.len()]
                }
                for column in &sample.columns{                  
                    graph_backlog[col_pos].push(match_value(column.value.clone()));
                    col_pos += 1;
                }
                
                let decimation_info = sampling_rates.get(&sample.stream.stream_id);
                if let Some(sampling) = decimation_info {
                    let fs = sampling[0] as f32/ sampling[1] as f32;
                    if fs >= 20.0 && graph_backlog.iter().all(|col| col.len() >= (fs / 20.0).ceil() as usize){
                        let _ = window.emit("main", &graph_backlog);
                        graph_backlog.iter_mut().for_each(|col| col.clear());
                    }
                    else {
                        let _ = window.emit("main", &graph_backlog);
                        graph_backlog.iter_mut().for_each(|col| col.clear());
                    }
                } 
            }
        }
    });
}


#[tauri::command]
fn fft_data(window: Window) {
    let args: Vec<String> = env::args().collect();   
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, &args);

    let stream_id: u8 = if args.len() >1 {
        match args[1].parse(){
            Ok(val) => val,
            Err(_e) => 1
        }
    } else {1};

    thread::spawn(move || {
        let proxy = proxy::Interface::new(&root);
        let device = proxy.device_full(route).unwrap();
        let mut device = Device::new(device);
        
        //get sampling & decimation rate, column metadata
        let meta = device.get_metadata();
        let mut sampling_rates: HashMap< u8, Vec<u32>> = HashMap::new();
        for stream in meta.streams.values() {
            sampling_rates.insert(stream.stream.stream_id, vec![stream.segment.sampling_rate, stream.segment.decimation]);
        }

        let elapsed = std::time::Instant::now();
        let mut fft_signals: HashMap<String, Vec<f32>> = HashMap::new();
        let mut calculate: bool = true;
        let ratelimiter = Ratelimiter::builder(1000, Duration::from_millis(100)).max_tokens(1000).build().unwrap();

        //Emitting FFT Data
        loop{ 
            let sample = device.next();
            let mut fft_freq: HashMap<String, Vec<f32>> = HashMap::new();
            let mut fft_power: HashMap<String, Vec<Vec<f32>>> = HashMap::new();

            if sample.stream.stream_id == stream_id{
                if let Err(sleep) = ratelimiter.try_wait() {
                        std::thread::sleep(sleep);
                        continue;
                }
                for column in &sample.columns{                  
                    if fft_signals.iter().all(|(_col, value)| value.len() >= 200) {
                        fft_signals.iter_mut().for_each(|(_col, value)| {value.remove(0);});
                    }
                    fft_signals.entry(column.desc.name.clone()).or_default().push(match_value(column.value.clone()));

                    if elapsed.elapsed() >= Duration::from_secs(1) && calculate {
                        let (freq, power) = calc_fft(fft_signals.get(&column.desc.name.clone()), sampling_rates.get(&sample.stream.stream_id));
                        if !freq.is_empty() && !power.is_empty() && !freq.iter().any(|&x| x.is_nan()) && !power.iter().any(|&x| x.is_nan()){
                            let parts: Vec<&str> = column.desc.name.split('.').collect();
                            let prefix = parts[0].to_string();
                            fft_power.entry(prefix.clone()).or_default().push(power.clone());
                            fft_freq.entry(prefix.clone()).or_insert_with(||freq.clone());
                        } else{
                            calculate = false;
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
                        let _ = window.emit(&name.clone(), spectrum_data);
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
                .title("Twinleaf UI") 
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
