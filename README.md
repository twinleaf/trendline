# Overview
`Trendline` compiles the functionality inside [`twinleaf-rust`](https://github.com/twinleaf/twinleaf-rust) into a graphical interface. It is intended to be a software oscilloscope and companion tool for use with [Twinleaf devices](https://twinleaf.com). It targets desktop platforms with support on macOS, Linux, and Windows.

## Implemented Features
- Time series data visualization of device streams
- Periodogram data visualization of device streams
- CSV export of plotted data 
- RPC control


# Developer Setup

`npm run tauri dev` will launch the program on the root directory. Make sure to run `npm install` first.

`cargo test export_bindings` will export the Rust bindings to be used in the front end Typescript when in the src-tauri directory (run to update structures inside `shared.rs`)

# Application Architectural Overview
`Trendline` uses [`Tauri`](https://tauri.app) to create a desktop application by combining a Rust backend with frontend frameworks (in our case, [`Svelte`](https://svelte.dev)). The corresponding `src-tauri` and `src` contain `README.md` detailing their architecture.

### Known Issues

- Multi series plots are notably slower than individual plots of the same series
- Typing in the RPC filtering slows down over time
- Changing sample rates from high sample rate to low sample rate truncates front-end buffers
- Stream monitor does not handle Time epoch type correctly and window statistics may fill zeros on non-time aligned data
- Seems to randomly drop sample packets during acquisition. ~~Could be caused by IPC back pressure?~~ Known issue on macOS with process priority.