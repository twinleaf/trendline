// src/proxy/discovery.rs
use crate::state::proxy_register::ProxyRegister;
use crate::util::get_valid_twinleaf_serial_urls;

use std::{collections::HashSet, sync::Arc, thread, time::Duration};

/// Spawn a background thread that keeps `ProxyRegister` in sync with whatever Twinleaf-compatible serial devices are visible on the host.
pub fn spawn(registry: Arc<ProxyRegister>) {
    thread::Builder::new()
        .name("twinleaf-discovery".into())
        .spawn(move || loop {
            let urls: HashSet<String> = get_valid_twinleaf_serial_urls().into_iter().collect();

            for url in &urls {
                registry.ensure(url.clone());
            }
            registry.prune(|url| urls.contains(url));

            thread::sleep(Duration::from_secs(2));
        })
        .expect("spawn discovery thread");
}
