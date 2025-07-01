export type DialogType = 'none' | 'discovery' | 'rpc_settings' | 'export';

class UiState {
	dialog = $state<DialogType>('none');

	open(type: DialogType)  { this.dialog = type; }
	close()                 { this.dialog = 'none'; }

	is(type: DialogType)    { return this.dialog === type; }
}

export const uiState = new UiState();
