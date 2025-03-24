# Trendline

Graphical interface tool for Twinleaf sensor stream data. Equipped with RPC controls and fast fourier transform analysis.

## Building

First, [install the Rust toolchain.](https://doc.rust-lang.org/cargo/getting-started/installation.html).
On Linux, install the [prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system.

Navigate to the src-tauri folder and run using the following command. Optionally specify the calculated FFT time duration (default 10 seconds) and refresh frequency (default 3 seconds) :

    cargo run [time_span] [refresh_rate]
