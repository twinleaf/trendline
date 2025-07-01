import { listen } from '@tauri-apps/api/event';
import type { PortState } from '$lib/bindings/PortState';
import type { UiDevice } from '$lib/bindings/UiDevice';
import { SvelteMap } from 'svelte/reactivity';
import { invoke } from '@tauri-apps/api/core';


export interface Selection {
	portUrl: string;
	childrenRoutes: string[];
}

class DeviceState {
	#devicesMap = new SvelteMap<string, { state: PortState; devices: UiDevice[]; }>;
	selection = $state<Selection | null>(null);

	constructor() {
		this.#initializeState();

		listen<[string, PortState]>('port-state-changed', ({ payload: [url, st] }) =>
			this.#setPortState(url, st)
		);
		listen<UiDevice[]>('port-devices-discovered', ({ payload: new_devices }) => {
			if (new_devices.length === 0) return;

			const url = new_devices[0].url;
			console.log(`[DeviceState] Received batch of ${new_devices.length} devices for port ${url}`);

			const entry = this.#devicesMap.get(url) ?? { state: new_devices[0].state, devices: [] };

			const updatedEntry = { ...entry, devices: new_devices };
			this.#devicesMap.set(url, updatedEntry);

		});
		listen<string>('device-removed', ({ payload: url }) => this.#removeDevice(url));
	}

	async #initializeState() {
		try {
			const allCurrentDevices = await invoke<UiDevice[]>('get_all_devices');

			if (allCurrentDevices.length > 0) {
				console.log(`[DeviceState] Initial FETCH found ${allCurrentDevices.length} devices.`);

				const groupedByPort = new Map<string, UiDevice[]>();
				for (const device of allCurrentDevices) {
					if (!groupedByPort.has(device.url)) {
						groupedByPort.set(device.url, []);
					}
					groupedByPort.get(device.url)!.push(device);
				}

				for (const [url, devicesForPort] of groupedByPort.entries()) {
					const portState = devicesForPort[0]?.state;
					this.#devicesMap.set(url, { state: portState, devices: devicesForPort });
				}
			} else {
				console.log(`[DeviceState] Initial fetch found no connected devices.`);
			}
		} catch (e) {
			console.error('Failed to get initial device state from backend:', e);
		}
	}


	#setPortState(url: string, state: PortState) {
		const entry = this.#devicesMap.get(url) ?? { state, devices: [] };
		const updatedEntry = { ...entry, state };
        this.#devicesMap.set(url, updatedEntry);
	}

	#removeDevice(url: string) {
		this.#devicesMap.delete(url);
	}

    devices = $derived.by(() => Array.from(this.#devicesMap.values()));

	deviceTree = $derived(() => {
		const out: (UiDevice & { children: UiDevice[] })[] = [];
		for (const [url, { devices }] of this.#devicesMap.entries()) {
			const parent = devices.find((d) => d.route === '/' || d.route === '');
			if (!parent) continue;

			const children = devices
				.filter((d) => d.route !== '/' && d.route !== '')
				.slice()
				.sort((a, b) => parseInt(a.route.slice(1), 10) - parseInt(b.route.slice(1), 10));

			out.push({ ...parent, children });
		}
		return out.sort((a, b) => a.url.localeCompare(b.url));
	});

	getPort(url: string) {
		return this.#devicesMap.get(url);
	}

    selectedDevices = $derived.by(() => {
		const sel = this.selection;
		if (!sel) return [];

		const portData = this.#devicesMap.get(sel.portUrl);
		if (!portData) return [];

		return portData.devices.filter(d => {
			const isParent = d.route === '/' || d.route === '';
			const isSelectedChild = sel.childrenRoutes.includes(d.route);
			return isParent || isSelectedChild;
		});
	});

	selectedPortState = $derived.by((): PortState | null => {
		const sel = this.selection;
		if (!sel) {
			return null;
		}

		const portData = this.#devicesMap.get(sel.portUrl);

		return portData ? portData.state : null;
	});

}

export const deviceState = new DeviceState();