class UiStore {
	discoveryDialogOpen = $state(true);

	openDiscoveryDialog() {
		this.discoveryDialogOpen = true;
		console.log('UI Store: Discovery dialog opened.');
	}
	closeDiscoveryDialog() {
		this.discoveryDialogOpen = false;
		console.log('UI Store: Discovery dialog closed.');
	}
}

export const uiStore = new UiStore();