// src/lib/services/device.store.svelte.ts

import { listen } from '@tauri-apps/api/event';
import type { PortState } from '$lib/bindings/PortState';
import type { UiDevice } from '$lib/bindings/UiDevice';

export interface UiDeviceWithKids extends UiDevice {
	childrenSorted: UiDevice[];
}

interface DeviceDetail {
	key: string;        
	name: string;
	route: string;
}

class DeviceService {
	#devicesMap = $state<Map<string, { state: PortState; devices: UiDevice[] }>>(new Map());

	constructor() {
		listen<[string, PortState]>('port-state-changed', ({ payload: [url, st] }) =>
			this.#setPortState(url, st)
		);
		listen<UiDevice>('new-device-meta-obtained', e => {
            console.log('[tauri] new-device-meta-obtained', e.payload);   // <—
            this.#addOrUpdateDevice(e.payload);
        });
		listen<string>('device-removed', ({ payload: url }) => this.#removeDevice(url));
	}

	#setPortState(url: string, state: PortState) {
		const entry = this.#devicesMap.get(url) ?? { state, devices: [] };
		entry.state = state;
		this.#devicesMap.set(url, entry);
	}

	#addOrUpdateDevice(newDevice: UiDevice) {
		const url = newDevice.url;
		const entry = this.#devicesMap.get(url) ?? { state: newDevice.state, devices: [] };
		const existingDeviceIndex = entry.devices.findIndex((d) => d.route === newDevice.route);

		if (existingDeviceIndex !== -1) {
			entry.devices[existingDeviceIndex] = newDevice;
		} else {
			entry.devices.push(newDevice);
		}
		entry.state = newDevice.state;
		this.#devicesMap.set(url, entry);
	}

	#removeDevice(url: string) {
		this.#devicesMap.delete(url);
	}

    devices = $derived(() => Array.from(this.#devicesMap.values()));

	deviceTree = $derived((): UiDeviceWithKids[] => {
		const out: UiDeviceWithKids[] = [];
		for (const { devices } of this.#devicesMap.values()) {
			const byUrl = new Map<string, UiDevice[]>();
			for (const d of devices) {
				const arr = byUrl.get(d.url) ?? [];
				arr.push(d);
				byUrl.set(d.url, arr);
			}
			for (const group of byUrl.values()) {
				const root = group.find((d) => d.route === '/' || d.route === '') ?? group[0];
				if (!root) continue;
				const children = group
					.filter((d) => d !== root)
					.slice()
					.sort((a, b) => parseInt(a.route.slice(1), 10) - parseInt(b.route.slice(1), 10));
				out.push({ ...root, childrenSorted: children });
			}
		}
		return out;
	});

	getPort(url: string) {
		return this.#devicesMap.get(url);
	}

	selection = $state<{ parent: string; children: string[] } | null>(null);

	selectedPortUrl = $derived(() => this.selection?.parent ?? null);

	selectedPortState = $derived(() => {
		const url = this.selectedPortUrl();
		return url ? this.#devicesMap.get(url)?.state ?? null : null;
	});

    selectedDeviceDetails = $derived((): DeviceDetail[] => {
        const sel = this.selection;
        if (!sel) return [];

        const port = this.#devicesMap.get(sel.parent);
        if (!port) return [];

        return port.devices
            .filter(d =>
                d.route === sel.parent || sel.children.includes(d.route)
            )
            .map(d => ({
                key:  `${d.url}${d.route}`,
                name: d.meta.name,
                route: d.route
            }));
    });

}

export const deviceService = new DeviceService();