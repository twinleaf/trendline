
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

