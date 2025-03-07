# Trendline
Graphical interface tool for Twinleaf sensor stream data. Equipped with RPC controls and fast fourier transform analysis

### Installation
[To install Rust and Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

Be sure to confirm all twinleaf-rust crate dependencies are satisfied.

To run: 

Connect the sensor to a proxy via twinleaf-rust proxy tool

    tio-proxy --auto

When there is more than one serial port available, it is necessary to specify the port

    [linux]> tio-proxy -r /dev/ttyACM0
	[macOS]> tio-proxy -r /dev/cu.usbserialXXXXXX
	[wsl1] > tio-proxy -r COM3

To run the program in the src-tauri folder:
    cargo run 