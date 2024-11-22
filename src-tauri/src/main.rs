#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tio::proto::DeviceRoute;
use tio::proxy;
use tio::util;
use twinleaf::tio;
use getopts::Options;
use bytemuck::cast_slice;
use twinleaf::data::{ColumnData, Device};

use tauri::{Window, Manager};
use std::time::Instant;
use std::thread;
use std::env;

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

fn get_serial() -> String{
    let ports = serialport::available_ports().expect("No ports found");
    let mut port = String::new();
    //iterates through possible serial ports
    for p in ports {
        //matches to macOS usbserial 
        if p.port_name.starts_with("/dev/cu.usb"){
            println!("Connecting to:{}", p.port_name);
            port = p.port_name
        }
    }
    port
}


#[tauri::command]
fn graphs(window: Window) {
    let args: Vec<String> = env::args().collect();
    //let mut args: Vec<String> = vec!["-r".to_string()];
    //let value = get_serial();
    //push the found serial to string
    //args.push(value.to_string());
    //args.push("-s".to_string());
    //args.push("/0".to_string());    
    
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, &args);
    
    let proxy = proxy::Interface::new(&root);
    
    let device = proxy.device_full(route).unwrap();
    //let devname: String = device.get("dev.name").unwrap();
    let devname: String = "USBSERIAL".to_string();
    let mut device = Device::new(device);

    let mut names: Vec<String> = Vec::new();
    if devname.as_str() == "VMR" {
        let column: String = device.get("data.stream.columns").unwrap(); // returns string of stream column names
        for name in column.split_whitespace() {
            names.push(name.to_string());
        }
    }

    thread::spawn(move || { 
        //let (_tx, rx) = proxy.full_port().unwrap();
        let start_time = Instant::now();
        
        match devname.as_str(){
            "VMR" => {
                for pkt in proxy.tree_full().unwrap().iter() {
                    if let tio::proto::Payload::LegacyStreamData(ref data) = pkt.payload {
                        let floats: &[f32] = cast_slice(&data.data);
                        let elapsed = start_time.elapsed().as_secs_f32();

                        let _ = window.emit("graphing", (&floats, &names, elapsed));
                    }
                }
            }
            //armstrong temp 
            "USBSERIAL" => {
                let mut current_name: String = String::new();
                loop{
                    let sample = device.next();
                    let mut names: Vec<String> = Vec::new();
                    let mut values: Vec<f32> = Vec::new();
            
                    match sample.stream.stream_id{
                        1 => {
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
                            //let _ = window.emit("graphing", (&values, &names));

                        }
                        0x02 => {
                            for column in &sample.columns{
                                names.push(column.desc.name.clone());
                                values.push(match column.value {
                                    ColumnData::Int(x) => x as f32,
                                    ColumnData::UInt(x) => x as f32,
                                    ColumnData::Float(x) => x as f32,
                                    ColumnData::Unknown => 0.0,
                                });
                            }
                            let _ = window.emit("graphing", (&values, &names));
        
                            
                        }
                        _ => {}
   
                    };
                   
                    for (name, value) in names.iter().zip(values.iter()){
                        println!("{}: {}", name, value);
                    }
                    //let _ = window.emit("graphing", (&values, &names));
                }
                
            }
            &_ => {} 
        }
            
    }); 
}
  
fn main(){
    tauri::Builder::default()
        .setup(|app| {
            let _window = app.get_window("main").unwrap();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            graphs])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
}