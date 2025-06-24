import { invoke } from '@tauri-apps/api/core';
import type { FeDeviceMeta } from '$lib/bindings/FeDeviceMeta';
import type { PortInfo } from '$lib/bindings/PortInfo';

class DeviceStore {
	isDeviceConnected = $state(false);
	isOccupied = $state(false);
	isStreaming = $state(false);
	selectedPorts = $state<PortInfo[]>([]);
	allDevices = $state<FeDeviceMeta[]>([]);

	async startStreaming(ports: PortInfo[], devices: FeDeviceMeta[]) {
		console.log('Device Store: Attempting to start streaming on backend...');
		try {
			await invoke('start_streaming', { ports });
			console.log('Device Store: Backend command successful.');

			this.selectedPorts = ports;
			this.allDevices = devices;
			this.isDeviceConnected = true;
			this.isOccupied = false;
			this.isStreaming = true;
			console.log('Device Store: Frontend state updated. Streaming is live.');
		} catch (error) {
			console.error('Device Store: Failed to start streaming on backend:', error);
			this.disconnect();
			throw error;
		}
	}

	stopStreaming() {
		this.isStreaming = false;
		console.log('Device Store: Streaming stopped.');
	}

	disconnect() {
		this.isDeviceConnected = false;
		this.isOccupied = false;
		this.isStreaming = false;
		this.selectedPorts = [];
		console.log('Device Store: Disconnected.');
	}
}

export const deviceStore = new DeviceStore();