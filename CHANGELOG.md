
# Change Log
All notable changes to this project will be documented in this file.
 
The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).
 

## [1.0.1] - 2025-09-10
  
 
### Added

- Ability to manually refresh devices from `DeviceSelectorDialog.svelte`
- Close button for `DeviceSelectorDialog.svelte`

### Changed
- Remove `handleKeyDown` event for `DeviceList.svelte` (changed tab index)
- Initially set current `PortState` to fetched parent level devices (instead of waiting for after selection)
- Change `detrend.rs` to push partial windows (to have periodogram plot even when there are not enough samples to populate full window)

### Fixed
- Fix `detrend.rs` to `Hydrate` exact number of samples (previously subsequent power of two)
    - Should fix periodogram retaining values outside of actual window seconds

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

