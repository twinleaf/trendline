// src/proxy/discovery.rs
use crate::shared::PortState;
use crate::state::proxy_register::ProxyRegister;
use crate::util::get_valid_twinleaf_serial_urls;

use std::{collections::HashSet, sync::Arc, thread, time::Duration};

const LOCAL_TCP_URL: &str = "tcp://localhost";
const DISCOVERY_POLL_INTERVAL: Duration = Duration::from_millis(500);

fn read_port_state(registry: &ProxyRegister, url: &str) -> Option<PortState> {
    let pm = registry.get(&url.to_string())?;
    pm.state.lock().ok().map(|s| s.clone())
}

fn tcp_is_online(state: &PortState) -> bool {
    matches!(
        state,
        PortState::Discovery | PortState::Streaming | PortState::Reconnecting
    )
}

/// Spawn a background thread that keeps `ProxyRegister` in sync with whatever Twinleaf-compatible serial devices are visible on the host.
pub fn spawn(registry: Arc<ProxyRegister>) {
    thread::Builder::new()
        .name("twinleaf-discovery".into())
        .spawn(move || {
            let mut tcp_seen_online = false;
            let tcp_url = LOCAL_TCP_URL.to_string();

            loop {
                registry.ensure(tcp_url.clone());

                if let Some(tcp_pm) = registry.get(&tcp_url) {
                    let state = tcp_pm.state.lock().ok().map(|s| s.clone());
                    if matches!(state, Some(PortState::Error(_) | PortState::Disconnected)) {
                        tcp_pm.connect();
                    }
                }

                let tcp_state = read_port_state(&registry, LOCAL_TCP_URL);
                let tcp_online_now = tcp_state.as_ref().is_some_and(tcp_is_online);

                if tcp_online_now {
                    tcp_seen_online = true;
                    // Multiplexer owns serial; avoid taking ports directly.
                    registry.shutdown_all_except(LOCAL_TCP_URL);
                    registry.prune(|_| false);
                    thread::sleep(DISCOVERY_POLL_INTERVAL);
                    continue;
                }

                let selected_port = registry.selected();
                if selected_port
                    .as_ref()
                    .is_some_and(|url| url.starts_with("tcp://"))
                {
                    // Respect explicit network selection: don't claim serial ports in the background.
                    registry.prune(|_| false);
                    thread::sleep(DISCOVERY_POLL_INTERVAL);
                    continue;
                }

                if let Some(selected_url) = selected_port.filter(|s| s.starts_with("serial://")) {
                    registry.ensure(selected_url.clone());
                    registry.prune(|url| url == &selected_url);
                    thread::sleep(DISCOVERY_POLL_INTERVAL);
                    continue;
                }

                let serial_fallback_allowed = !tcp_seen_online;

                if serial_fallback_allowed {
                    let urls: HashSet<String> = get_valid_twinleaf_serial_urls()
                        .into_iter()
                        .filter(|url| !registry.is_serial_ignored(url))
                        .collect();
                    for url in &urls {
                        registry.ensure(url.clone());
                    }
                    registry.prune(|url| urls.contains(url));
                } else {
                    registry.prune(|_| false);
                }

                thread::sleep(DISCOVERY_POLL_INTERVAL);
            }
        })
        .expect("Failed to spawn Discovery thread.");
}
