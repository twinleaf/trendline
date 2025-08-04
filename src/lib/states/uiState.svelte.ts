import { listen } from '@tauri-apps/api/event';
import { toast } from "svelte-sonner";

export type DialogType = 'none' | 'discovery' | 'rpc_settings' | 'export';

class UiState {
	dialog = $state<DialogType>('none');

	constructor() {
		// Listen for the success event from the Rust backend
		listen<string>('csv-export-complete', ({ payload }) => {
			this.showSuccess(payload);
		});
	}

	// --- Public methods for showing different types of toasts ---
	showSuccess(message: string) {
		toast.success(message);
	}

	showError(message: string) {
		toast.error(message);
	}

	showInfo(message: string) {
		toast.info(message);
	}

	showWarning(message: string) {
		toast.warning(message);
	}

	open(type: DialogType) {
		this.dialog = type;
	}
	close() {
		this.dialog = 'none';
	}

	is(type: DialogType) {
		return this.dialog === type;
	}
}

export const uiState = new UiState();