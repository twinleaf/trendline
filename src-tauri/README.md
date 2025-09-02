## System Design


Single‑process backend (Tauri/Rust) that:

* owns device state and time‑series,
* runs per‑port I/O actors,
* drives a small processing pipeline graph for plots,
* talks to the UI via Tauri commands + IPC channels
* updates state changes via Tauri `Emitters`

### Core components & contracts

**PortManager (per serial/TCP port)**

* Connects → discovers → streams. Emits `port-state-changed`, `port-devices-discovered`, `device-metadata-updated`.
* On reconnect, re‑applies the last selection from `ProxyRegister.active_selections`.
* Pushes samples as `InsertBatch` only for *active* columns; updates stream sample rates on metadata changes.

**CaptureState (Time-series Data Base)**

* Rolling window ≈ **180 s** per `(DataColumnId, SessionId)` in a `BTreeMap`; capacity scales with effective sample rate.
* Aligns multiple device sessions onto a **unified time** using host `Instant` gaps (not device clocks).
* Fans out raw batches to subscribers (pipelines). Supports **snapshots** for paused plots.
* Only keys in `active` are recorded (updated by frontend via. Tauri command)

**ProcessingManager (pipelines + emitter)**

* Spawns **root** pipelines (subscribe to raw batches) and **derived** ones (subscribe to other pipelines).
* Emits merged `PlotData` to the UI roughly every **33 ms** (k‑way merge + linear interp; `NaN` for gaps).
* Backpressure: root channels **128**; derived channels **1** (drop/overwrite vs. backlog).
* Built‑ins: `Passthrough`, `FPCS` decimation, windowed `Detrend` (None/Linear/Quadratic), `FFT` (Welch→ASD), and streaming **Statistics** (window + persistent).

### Data flow

1. UI: `connect_to_port(url)` → `PortManager` starts.
2. UI selects routes → `confirm_selection` → `CaptureState.SetActiveColumns`.
3. UI: `update_plot_pipeline(SharedPlotConfig)` → manager spawns pipelines, hydrates from `CaptureState`.
4. UI: `listen_to_plot_data(plot_id, channel)` → manager emits \~30 FPS.
5. Optional: stats provider + channel.
6. Optional: `pause_plot` → export snapshot/raw CSV.

### Non‑obvious behaviors

* Time alignment across sessions is **host‑time‑based**; device time discontinuities won’t break plots.
* If you wait > window (\~180 s), **raw export** for a live plot will error (data rolled out).
* CSV headers are `route.column` when multiple devices are present; integers are formatted losslessly.
* `Hydrate` is important: pipelines size buffers from effective sample rate and backfill the window.

### Adding a new Pipeline

* Implement `Pipeline`, handle `Hydrate`, expose output via `get_output()`.
* Spawn via `spawn_root_pipeline` (raw) or `spawn_derived_pipeline` (from another node).
* Wire it in `apply_plot_config` and add its output to the plot’s `output_pipeline_ids`.