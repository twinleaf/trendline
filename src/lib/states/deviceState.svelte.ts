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

class DeviceState {
	#devicesMap = $state<Map<string, { state: PortState; devices: UiDevice[] }>>(new Map());

	constructor() {
		listen<[string, PortState]>('port-state-changed', ({ payload: [url, st] }) =>
			this.#setPortState(url, st)
		);
		listen<UiDevice[]>('port-devices-discovered', ({ payload: new_devices }) => {
			if (new_devices.length === 0) return;

			const url = new_devices[0].url;
			console.log(`[DeviceState] Received batch of ${new_devices.length} devices for port ${url}`);

			const newMap = new Map(this.#devicesMap);
			const entry = newMap.get(url) ?? { state: new_devices[0].state, devices: [] };

			const updatedEntry = { ...entry, devices: new_devices };
			newMap.set(url, updatedEntry);

			this.#devicesMap = newMap;
		});
		listen<string>('device-removed', ({ payload: url }) => this.#removeDevice(url));
	}

	#setPortState(url: string, state: PortState) {
		const newMap = new Map(this.#devicesMap);
		const entry = newMap.get(url) ?? { state, devices: [] };
		const updatedEntry = { ...entry, state };
        newMap.set(url, updatedEntry);
        this.#devicesMap = newMap;
	}

	#removeDevice(url: string) {
		const newMap = new Map(this.#devicesMap);
		newMap.delete(url);
		this.#devicesMap = newMap;
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
            .filter(d => {
				const isParent = d.route === '/' || d.route === '';
				const isSelectedChild = sel.children.includes(d.route);
				return isParent || isSelectedChild;
			})
            .map(d => ({
                key:  `${d.url}${d.route}`,
                name: d.meta.name,
                route: d.route
            }));
    });

}

export const deviceState = new DeviceState();