# Overview
`Trendline` compiles the functionality inside [`twinleaf-rust`](https://github.com/twinleaf/twinleaf-rust) into a graphical interface. It is intended to be a software oscilloscope and companion tool for use with [Twinleaf devices](https://twinleaf.com). It targets desktop platforms with support on MacOS, Linux, and Windows.

## Implemented Features
- Time series data visualization of device streams
- RPC control

## TODO
### User Interface
- Multi-device view (allows side-by-side device graphs)
### Data Streams
- Data logging (saving to binary format)
- Math channels (filters, addition/subtraction)
- Cursors (right click to place static cursor)
- Triggers (threshold-based pausing)

# Developer Setup

`npm run tauri dev` will launch the program on the root directory

`cargo test export_bindings` will export the Rust bindings to be used in the front end Typescript when in the src-tauri directory 

# Application Architectural Overview
`Trendline` uses [`Tauri`](https://tauri.app) to create a desktop application by combining a Rust backend with frontend frameworks (in our case, [`Svelte`](https://svelte.dev)). The corresponding `src-tauri` and `src` contain `README.md` detailing their architecture.