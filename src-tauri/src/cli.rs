///This is a basic example of using the twinleaf rust package to run a CLI
/// display of VMR stream data 
use twinleaf::tio;
use twinleaf::data::{ColumnData, Device, Sample};
use tio::proto::DeviceRoute;
use tio::proxy;
use tio::util;
mod utils;

use getopts::Options;
use std::env;

use bytemuck::cast_slice;
use pancurses::*;


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

fn dump_stream(args: &[String]) {
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, args);
    
    let proxy = proxy::Interface::new(&root);
    let device = proxy.device_rpc(route).unwrap();

    let column: String = device.get("data.stream.columns").unwrap(); //proxy get fn returns string of stream column names
    let mut names: Vec<String> = Vec::new();
   
    for name in column.split_whitespace() { //stream column names get pushed to a vector
        names.push(name.to_string());
    }

    //initialize terminal window
    let window = initscr();
    window.refresh();
    noecho();

    for pkt in proxy.tree_full().unwrap().iter() {
        if let tio::proto::Payload::LegacyStreamData(ref data) = pkt.payload {
            window.clear();
            let floats: &[f32] = cast_slice(&data.data);
            
            for (name, &value) in names.iter().zip(floats.iter()) { //iterate through the names and stream data
                println!("\n"); 
                window.refresh();                
                let string = format!("{}: {:?}", name.as_str(), value);               
                window.mvprintw(0,0, &string); //print string into window
            }   
        }
        
    }
    endwin();
}

//new metadata format
fn dump_armstrong(args: &[String]) {
    let opts = tio_opts();
    let (_matches, root, route) = tio_parseopts(opts, args);
    
    let proxy = proxy::Interface::new(&root);
    let device = proxy.device_full(route).unwrap();
    let mut device = Device::new(device);

    //initialize terminal window
    let window = initscr();
    
     
    start_color();
    init_pair(1, COLOR_WHITE, COLOR_BLACK);
    init_pair(2, COLOR_GREEN, COLOR_BLACK);
    init_pair(3, COLOR_RED, COLOR_BLACK);
    
    window.refresh();
    noecho();
    let mut pos = 4;
    
    loop{
        let sample: &Sample = &device.next();

        let name = format!("Device Name: {}  Serial: {}   Session ID: {}", sample.device.name, sample.device.serial_number, sample.device.session_id);
        window.mvprintw(1,0, &name);
        
        for col in &sample.columns{
            let color_pair = utils::range::thresh_test(col.desc.name.clone(), 
                match col.value {
                ColumnData::Int(x) => x as f32,
                ColumnData::UInt(x) => x as f32,
                ColumnData::Float(x) => x as f32,
                ColumnData::Unknown => 0.0,
                });
            
            
            match sample.stream.stream_id{
                0x01 => {
                    window.clear();
                    let string = format!(
                        "{}: {}",
                        col.desc.name,
                        match col.value {
                            ColumnData::Int(x) => format!("{}", x),
                            ColumnData::UInt(x) => format!("{:.3}", x),
                            ColumnData::Float(x) => format!("{:.3}", x),
                            ColumnData::Unknown => "?".to_string(),
                        }
                    );  
                    
                    window.attron(COLOR_PAIR(color_pair));
                    window.mvprintw(3, 0, &string);
                    
                    window.attroff(COLOR_PAIR(color_pair));
                }
                0x02 => {
                    window.refresh();
                    
                    let string = format!(
                        " {}: {}",
                        col.desc.name,
                        match col.value {
                            ColumnData::Int(x) => format!("{}", x),
                            ColumnData::UInt(x) => format!("{:.3}", x),
                            ColumnData::Float(x) => format!("{:.3}", x),
                            ColumnData::Unknown => "?".to_string(),
                        }
                    );  
                    window.attron(COLOR_PAIR(color_pair));
                    window.mvprintw(pos, 0, &string);                
                    window.attroff(COLOR_PAIR(color_pair));
                    window.refresh();
                    
                    pos += 1;
                }
                _ => {}
            }
        }
        pos = 4;
    }
}

fn main(){
    let args: Vec<String> = env::args().collect();

    match args[1].as_str() {
        "stream" => {
            dump_stream(&args[1..])
        }
        "arm" => {
            dump_armstrong(&args[1..])
        }
        _ => {}
    }

}






