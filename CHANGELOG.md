
# Change Log
All notable changes to this project will be documented in this file.
 
The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).
 

## [1.0.1] - 2025-09-10
  
 
### Added

- Ability to manually refresh devices from `DeviceSelectorDialog.svelte`
- Close button for `DeviceSelectorDialog.svelte`
- Fontsource npm module for bundling fonts in app distribution
- Drop `PortManager`s for non-selected URLs
- Added ability to clear individual columns and all columns via stream monitor
- Added NaN count and skipped sample number to `StreamMonitor` (toggle button to go to `HealthSet`)
- Added Linux ARM build
- Make `port_manager` send sample numbers alongside data to `CaptureState`

### Changed
- Revert discovery port from `tree_probe()` to `tree_full()`
    - Armstrong devices only transmit `StreamData` consistently, so nothing on probe shows routing info
- Remove `handleKeyDown` event for `DeviceList.svelte` (changed tab index)
- Initially set current `PortState` to fetched parent level devices (instead of waiting for after selection)
- Change `detrend.rs` to push partial windows (to have periodogram plot even when there are not enough samples to populate full window)
- Make `StreamMonitor` use channel descriptions as names by default (toggle inside settings pop over)
- Make default plot title from `StreamMonitor` use channel descriptions
- Make confirm button focus on valid state (not disabled)
- Clearing all stream monitors also clears all `CaptureState` buffers
- Changed `StreamMonitor` window from 2 seconds to 1 second
- Rename from `StreamStatistics` to `ColumnStatistics`
- Separate into `StatisticSet` into `HealthSet` and `StatisticSet`

### Fixed
- Fix `detrend.rs` to `Hydrate` exact number of samples (previously subsequent power of two)
    - Should fix periodogram retaining values outside of actual window seconds
- Fix infinite reactivity when moving plot and then resizing using a preset sizes
    - Occurred due to resize handler attempting to react to preset size change, in turn making the preset size react
- Fix name of plot getting cut off

## [1.0.0] - 2025-09-02

Initial release
 
### Added
- Visualize multiple data streams of uniform sample rate on a single chart
- Detrending for periodogram (mean removal, linear fit, quadratic fit)

### Changed
- Change front end to use Svelte 5 from native HTML
- Decouple [`twinleaf-rust`](https://github.com/twinleaf/twinleaf-rust) binary tools from project codebase 
- Fetch all RPCs on start up

### Fixed
- Fix crashing on device unplug by using proxy status channel

