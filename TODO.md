Project TODO List
This document outlines the current bugs, desired features, and code quality improvements for the Trendline application.

🐞 Bug Fixes

[ ] Mismatched Sample Rates: Investigate why selecting data streams with different sample rates results in NaN values, with behavior dependent on the selection order.

[ ] Cursor Behavior: Fix the plot cursor to correctly select the nearest non-null data point, instead of just the nearest x-coordinate.

[ ] FFT Data Source: Resolve the issue where the FFT calculation fails for data series that are not the primary X component (column index 0) and for IMU data streams.

[ ] Plot Resizing: Determine why adding a new plot to the view causes all existing plots to shrink slightly.

[ ] Proxy Reconnect Loop: Fix the bug where a reconnecting proxy spawns a new, independent proxy instance, leading to multiple reconnection timers.

✨ Features

[ ] Scalar Data Viewer: Implement exponential moving average and a boxed display for scalar values

[ ] Device Selector UI: Implement the pop-up modal for device selection, triggered from the "Change Device" menu item.

[ ] Device Selector manual URL input: Implement text entry option for device selection.

[ ] RPC Settings Panel: Create a dedicated UI for viewing and modifying device RPC parameters.

[ ] Plot Settings Tab: Add a tab panel within the settings modal (cog wheel) for adjusting plot-specific configurations (eg. colors, disable/enable plot title, window length)

[ ] Channel Search Bar: Implement a search bar in the channel selection interface to allow users to quickly filter available data streams.

[ ] Plot Reordering: Add functionality to allow users to drag and drop plots to reorder their display.

[ ] Data Logging: Add functionality to record binary data stream and eventually restream data as a headless device

[ ] TCP Proxy: Replicate functionality in `tio-proxy` as a checkbox on device discovery and in the MenuBar toggle setting

🛠️ Code Quality & Refactoring

[ ] Reactive Data Flow:

Goal: Transition from a polling-based model to a reactive, event-driven architecture.

Implementation Idea: Instead of a single confirm_selection command, use atomic backend events (connect_stream, disconnect_stream). The frontend plots would subscribe to the relevant data streams, and the backend would push updates. This decouples the UI from managing connection state logic.

[ ] State Management Consolidation:

Goal: Clean up component-level TypeScript logic by moving it into the centralized chartState and deviceState stores.

Implementation Idea: Break down the monolithic chartState into smaller, modular uPlot configuration managers. This will simplify state management and make it easier to integrate with the new plot settings panels.

❓ Things to refactor

[ ] Move serialize `match rpc_type` into helper inside `twinleaf` crate
[ ] Move `RpcMeta` into... somewhere (maybe `twinleaf::meta` but it's not relevant to `proto`)
[ ] Move `device_enumerate()` into a `twinleaf` `util.rs` or something

