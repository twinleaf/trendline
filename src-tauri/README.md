## Backend Architecture (Tauri/Rust)
The backend is partitioned into two planes: a **control plane** and a **data plane**. This separation is to ensure low-frequency commands (control queries) do not interefere with high-frequency commands (data queries).

### Control Plane
> # [INSERT FINITE STATE MACHINE DIAGRAM]

**Principle**: Implemented as a single **Actor** running a **Finite State Machine (FSM)**, the Control Plane is the authoritative **Single Source of Truth (SSoT)** for the operational state of the system. It is *exclusively* responsible for the lifecycle management of all background processes. It also holds low-volume, structural, state, and configuration data.

**Responsibilities**:
- Executing commands that change the system's state (e.g., `DiscoverDevices`, `StopStreaming`, `ConfigureMathChannel`) which induce transitions between states (e.g., `FSM::Idle`, `FSM::Discovering`, `FSM::Streaming`)
- Spawning, monitoring, and terminating all long-running threads for data acquisition or processing
- Responding asynchronously to hardware events (e.g., disconnection). This triggers a corresponding FSM update (and if applicable, event notification)
- Answering queries about the system's state (e.g., `GetDeviceStatus`)

### Data Plane
**Principle**: A collection of passive data structures and *stateless* functions responsible for data throughput and transformation. It has no lifecycle of its own and is managed by the Control Plane. It holds high-volume, stremaing, time-series measurement data.

**Responsibilities**:
- Providing a thread-safe, in-memory buffer (`Arc<Mutex<TimeSeriesData>>`)
- Containing pure logic for data processing (e.g., `calculate_rms(series: &Vec<f64>) -> f64`)
- Exposing low-latency, read-only data access commands (`get_plot_data_in_range`) that bypasses the Control Plane's queue