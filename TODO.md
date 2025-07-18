# Project TODO List
This document outlines the current bugs, desired features, and code quality improvements for the Trendline application.


| Marker | Definition                |
|:------:|---------------------------|
|    -   | Temporary fix implemented |
|    X   | Permanent fix implemented |
|    O   | Deferred implementation   |


## Bug Fixes

[ ] Cursor Behavior: Fix the plot cursor to correctly select the nearest non-null data point, instead of just the nearest x-coordinate.

[ ] Plot Resizing: Determine why adding a new plot to the view causes all existing plots to shrink slightly.

[-] Proxy Reconnect Loop: Fix the bug where a reconnecting proxy spawns a new, independent proxy instance, leading to multiple reconnection timers.

[ ] Accessibility Behavior: Fix the bug where removing the explicit Chevron button in the Plot Selection means that it is not possible to expand the DataTable using only keyboard navigation

[ ] Plot Control Focus: Fix how plot control's focus is not removed when the PopOver disappears (eg. hitting space bar after closing the popover reopens it). Presumably tied to the button focus.

# #Features

[ ] Scalar Data Viewer: Implement exponential moving average and a boxed display for scalar values

[ ] Device Selector UI: Implement the pop-up modal for device selection, triggered from the "Change Device" menu item.

[ ] Channel Search Bar: Implement a search bar in the channel selection interface to allow users to quickly filter available data streams.

[ ] Plot Reordering: Add functionality to allow users to drag and drop plots to reorder their display.

[ ] Data Logging: Add functionality to record binary data stream and eventually restream data as a headless device

[ ] TCP Proxy: Replicate functionality in `tio-proxy` as a checkbox on device discovery and in the MenuBar toggle setting

[ ] Multi-device view: Allows side-by-side device graphs, possibly separated using a pagination or carousel

[ ] Menubar: Both integrating MacOS native and making a Svelte Menubar component

[ ] Context menu: Should be able to right click to add or move plots in the ChartView area

## Code Quality & Refactoring

[ ] Reactive Data Flow:

Goal: Transition from a polling-based model to a reactive, event-driven architecture.

Implementation Idea: Instead of a single confirm_selection command, use atomic backend events (connect_stream, disconnect_stream). The frontend plots would subscribe to the relevant data streams, and the backend would push updates. This decouples the UI from managing connection state logic.

[ ] State Management Consolidation:

Goal: Clean up component-level TypeScript logic by moving it into the centralized chartState and deviceState stores.

Implementation Idea: Break down the monolithic chartState into smaller, modular uPlot configuration managers. This will simplify state management and make it easier to integrate with the new plot settings panels.

## Things to refactor

[ ] Move serialize `match rpc_type` into helper inside `twinleaf` crate

[ ] Move `RpcMeta` into... somewhere (maybe `twinleaf::meta` but it's not relevant to `proto`)

[ ] Move `device_enumerate()` into a `twinleaf` `util.rs` or something
