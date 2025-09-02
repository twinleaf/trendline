import { listen } from '@tauri-apps/api/event';
import { toast } from "svelte-sonner";
import { revealItemInDir } from '@tauri-apps/plugin-opener';

export type DialogType = 'none' | 'discovery' | 'rpc_settings' | 'export';

interface CsvExportPayload {
    message: string;
    path: string | null;
}

class UiState {
	dialog = $state<DialogType>('none');

	constructor() {
		listen<CsvExportPayload>('csv-export-complete', ({ payload }) => {
			if (payload.path) {
				this.showFileSaveSuccess(payload.message, payload.path);
			} else {
				this.showSuccess(payload.message);
			}
		});
	}

	showFileSaveSuccess(message: string, path: string) {
		toast.success(message, {
			duration: 6000,
			action: {
				label: 'Show',
				onClick: () => revealItemInDir(path)
			},
		});
	}

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