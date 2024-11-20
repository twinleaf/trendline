use std::process::{Command, exit};
use std::str;

//Uses the connect tool from tio-proxy to listen in on the sensor

fn main() {
    let proxy_path = "/Users/krzywonos/Desktop/Rust/twinleaf-rust-dev/twinleaf-tools/";

    let output = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(format!("{}/Cargo.toml", proxy_path))
        .arg("--bin")
        .arg("tio-proxy")
        .arg("--")
        //.arg(format!("{}", value))
        .arg("--auto")
        .output()
        .expect("Failed to execute");

    if output.status.success() {
        println!("Proxy ran successfully")
        
    } else {
        eprintln!("Proxy failed to connect");
        match str::from_utf8(&output.stderr){
            Ok(stderr_str) => {
                eprintln!("stderr: {}", stderr_str)
            },
            Err(_) => {
                eprintln!("stderr: non-UTF-8 bytes");
                eprintln!("stderr: {:?}", output.stderr);
            }
        }
        exit(1);
    }
}



