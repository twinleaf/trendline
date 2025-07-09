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
            if (allCurrentDevices.length === 0) {
                console.log(`[DeviceState] Initial fetch found no connected devices.`);
                return;
            }
            console.log(`[DeviceState] Initializing with ${allCurrentDevices.length} total devices.`);

            const groupedByPort = new Map<string, UiDevice[]>();
            for (const device of allCurrentDevices) {
                if (!groupedByPort.has(device.url)) {
                    groupedByPort.set(device.url, []);
                }
                groupedByPort.get(device.url)!.push(device);
            }

            for (const [url, devicesForPort] of groupedByPort.entries()) {
                try {
                    const currentState = await invoke<PortState>('get_port_state', { portUrl: url });

                    this.#devicesMap.set(url, { state: currentState, devices: devicesForPort });
                    console.log(`[DeviceState] Hydrated port ${url} with state: ${JSON.stringify(currentState)}`);

                } catch (e) {
                    console.error(`[DeviceState] Failed to get real-time state for port ${url}. Using fallback.`, e);
                    const fallbackState = devicesForPort[0]?.state ?? 'Disconnected';
                    this.#devicesMap.set(url, { state: fallbackState, devices: devicesForPort });
                }
            }
        } catch (e) {
            console.error('Failed to get initial device list from backend:', e);
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

	deviceTree = $derived.by(() => {
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

	getDevice(portUrl: string, route: string): UiDevice | undefined {
        const portData = this.#devicesMap.get(portUrl);
        if (!portData) return undefined;

        return portData.devices.find(d => d.route === route);
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