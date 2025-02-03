# Lily
Graphical interface tool for Twinleaf sensor stream data. Equipped with RPC controls and fast fourier transform analysis

### Development
On linux, there is a dependency on libudev; to install it use:

	sudo apt install libudev-dev  # debian linux

### Installation
Be sure to confirm all twinleaf-rust crate dependencies are satisfied.

To run: 

Connect the sensor to a proxy via twinleaf-rust proxy tool

    tio-proxy --auto

When there are more than one serial port available, it is necessary to specify the port

    [linux]> tio-proxy -r /dev/ttyACM0
	[macOS]> tio-proxy -r /dev/cu.usbserialXXXXXX
	[wsl1] > tio-proxy -r COM3

Optionally specify stream number and fft sampling time (default is on stream 1 and a 10 second sampling time)

    cargo run <stream_id> <fft_time>
