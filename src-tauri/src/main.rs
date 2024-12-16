#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use twinleaf::{tio, data::{ColumnData, Device}};
use tio::{proto::DeviceRoute, proxy, util};
use getopts::Options;

use tauri::{Emitter, Listener, LogicalPosition, LogicalSize, WebviewUrl, Window, Manager};
use std::{env, thread};

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


#[tauri::command]
//main tauri function where stream data gets emitted to graphs on frontend
fn graphs(window: Window) {
    let args: Vec<String> = env::args().collect();   
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, &args);
    
    thread::spawn(move || {  
        let proxy = proxy::Interface::new(&root);
        let device = proxy.device_full(route.clone()).unwrap();
        let mut device = Device::new(device); 

        //armstrong temp 
        //TODO:: Need set up following flexible number of streams
        let mut current_name: String = String::new();
        loop{
            let sample = device.next();
            let header = format!("Connected to: {}   Serial: {}   Session ID: {}", sample.device.name, sample.device.serial_number, sample.device.session_id);
            let mut names: Vec<String> = Vec::new();
            let mut values: Vec<f32> = Vec::new();
    
            match sample.stream.stream_id{
                3 => {
                    for column in &sample.columns{
                        let name = column.desc.name.clone();
                        if name != current_name && !(names.contains(&name)){
                            names.push(column.desc.name.clone());
                            current_name = name.clone();
                        }
                        values.push(match column.value {
                            ColumnData::Int(x) => x as f32,
                            ColumnData::UInt(x) => x as f32,
                            ColumnData::Float(x) => x as f32,
                            ColumnData::Unknown => 0.0,
                        });
                    }
                    let _ = window.emit("field", (&values, &names, &header));
                }
                2 => {
                    for column in &sample.columns{
                        names.push(column.desc.name.clone());
                        values.push(match column.value {
                            ColumnData::Int(x) => x as f32,
                            ColumnData::UInt(x) => x as f32,
                            ColumnData::Float(x) => x as f32,
                            ColumnData::Unknown => 0.0,
                        });
                    }
                    let _ = window.emit("graphing", (&values, &names, &header));                         
                }
                _ => {}
            };
            /*for (name, value) in names.iter().zip(values.iter()){
                println!("{}: {}", name, value);
            }*/
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

            //webviews within main window
            let aux = window.add_child(
                tauri::webview::WebviewBuilder::new("aux", WebviewUrl::App(Default::default()))
                    .auto_resize(),
                    LogicalPosition::new(0., 0.),
                    LogicalSize::new(800., 600.),
                )?;

            let desc = window.add_child(
                tauri::webview::WebviewBuilder::new("desc", WebviewUrl::App("desc.html".parse().unwrap()),)
                .auto_resize(),
                LogicalPosition::new(0., 0.),
                LogicalSize::new(800., 600.),
                )?;

            let field = tauri::WebviewWindowBuilder::new(app, "PowerMon", WebviewUrl::App("field.html".parse().unwrap()))
                .title("Field")
                .inner_size(750., 550.)
                .build()?;

            //listen on backend for which webview to show
            aux.show().unwrap();
            desc.hide().unwrap();
            field.hide().unwrap();

            app.listen("toggle",  move |event| {
                let webpage: String = serde_json::from_str(event.payload()).unwrap();

                match webpage.as_str() {
                    "lily" => {
                        aux.show().unwrap();
                        desc.hide().unwrap();
                    }
                    "desc" => {
                        aux.hide().unwrap();
                        desc.show().unwrap();
                    }
                    "field" => {
                        if field.is_visible().expect("Not visible") {
                            field.hide().unwrap();
                        } else {
                            field.show().unwrap();
                        }                        
                    }
                    _ => {}
                }
            }); 

            //TODO: simplify logic between listening on load and on a regular logic
            //App listens for the RPC call and returns the result to the specified window
            let main_window = app.get_webview("aux").unwrap();
            let new_window = app.get_webview("aux").unwrap();
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
                    let _ = new_window.emit("returnOnLoad", (rpccall.clone(), passed.clone()));
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            graphs])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
}
