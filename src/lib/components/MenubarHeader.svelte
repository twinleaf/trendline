<script lang="ts">
	import * as Menubar from '$lib/components/ui/menubar/';
	import { deviceService } from '$lib/stores/device.store.svelte';
	import { ioStore } from '$lib/stores/io.store.svelte';

	// --- MOCK APPLICATION STATE (can stay for now) ---
	let channelViewMode = $state('overlaid');
	let showCursors = $state(false);

	// --- REMOVE ALL THE `handle...` FUNCTIONS ---
	// They are no longer needed as we will call the stores directly.
	
	function handleOpenRecording() { console.log('TODO: Open Recording'); }
	function handleSaveRecording() { console.log('TODO: Save Recording'); }
	function handleExport(format: 'csv' | 'png') { console.log(`TODO: Export as ${format}`); }
	function handleCopy(type: 'screenshot' | 'data') { console.log(`TODO: Copy ${type}`); }
    function handleOpenDeviceSettings() { console.log('TODO: Open RPC Settings'); }

</script>

<Menubar.Root class="bg-card">
	<!-- FILE MENU -->
	<Menubar.Menu>
		<Menubar.Trigger>File</Menubar.Trigger>
		<Menubar.Content>
			<Menubar.Item onSelect={handleOpenRecording}>
				Open Recording... <Menubar.Shortcut>⌘O</Menubar.Shortcut>
			</Menubar.Item>
			<Menubar.Item onSelect={handleSaveRecording} disabled={!deviceStore.isDeviceConnected}>
				Save Recording As... <Menubar.Shortcut>⌘S</Menubar.Shortcut>
			</Menubar.Item>
			<Menubar.Sub>
				<Menubar.SubTrigger>Export As...</Menubar.SubTrigger>
				<Menubar.SubContent>
					<Menubar.Item onSelect={() => handleExport('csv')}>CSV Data (.csv)</Menubar.Item>
					<Menubar.Item onSelect={() => handleExport('png')}>Screenshot (.png)</Menubar.Item>
				</Menubar.SubContent>
			</Menubar.Sub>
			<Menubar.Separator />
			<Menubar.Item>Preferences...</Menubar.Item>
			<Menubar.Separator />
			<Menubar.Item>Exit</Menubar.Item>
		</Menubar.Content>
	</Menubar.Menu>

	<!-- EDIT MENU -->
	<Menubar.Menu>
		<Menubar.Trigger>Edit</Menubar.Trigger>
		<Menubar.Content>
			<Menubar.Item onSelect={() => handleCopy('screenshot')}>
				Copy Screenshot <Menubar.Shortcut>⇧⌘C</Menubar.Shortcut>
			</Menubar.Item>
			<Menubar.Item onSelect={() => handleCopy('data')}>Copy Data (CSV)</Menubar.Item>
			<Menubar.Separator />
			<Menubar.Item onSelect={() => chartStore.clearAllCharts()} disabled={deviceStore.isConnecting}>
				Clear Session
			</Menubar.Item>
		</Menubar.Content>
	</Menubar.Menu>

	<!-- VIEW MENU -->
	<Menubar.Menu>
		<Menubar.Trigger>View</Menubar.Trigger>
		<Menubar.Content>
			<Menubar.RadioGroup bind:value={channelViewMode}>
				<Menubar.GroupHeading>Arrange Channels</Menubar.GroupHeading>
				<Menubar.RadioItem value="overlaid">Overlaid</Menubar.RadioItem>
				<Menubar.RadioItem value="stacked">Stacked</Menubar.RadioItem>
			</Menubar.RadioGroup>
			<Menubar.Separator />
			<Menubar.CheckboxItem bind:checked={showCursors}>Show Cursors</Menubar.CheckboxItem>
			<Menubar.Item inset>Show Current Values</Menubar.Item>
			<Menubar.Item inset>Show Controls</Menubar.Item>
			<Menubar.Separator />
			<Menubar.Item inset>Zoom In</Menubar.Item>
			<Menubar.Item inset>Zoom Out</Menubar.Item>
			<Menubar.Item inset>Zoom to Fit</Menubar.Item>
		</Menubar.Content>
	</Menubar.Menu>

	<!-- DEVICE MENU -->
	<Menubar.Menu>
		<Menubar.Trigger>Device</Menubar.Trigger>
		<Menubar.Content>
			<!-- FIX: Use arrow function to call ioStore.toggleLogging -->
			<Menubar.Item onSelect={() => ioStore.toggleLogging()} disabled={!deviceStore.isDeviceConnected}>
				{#if ioStore.isLogging}
					Stop Logging
				{:else}
					Start Logging
				{/if}
				<Menubar.Shortcut>⌘Space</Menubar.Shortcut>
			</Menubar.Item>
			<Menubar.Separator />
			<!-- FIX: Use arrow function to call store methods -->
			<Menubar.Item onSelect={() => {
				if (deviceStore.isDeviceConnected) {
					// We might want to disconnect first before opening the dialog
					deviceStore.disconnect(); 
				}
				uiStore.openDiscoveryDialog();
			}}>
				{#if deviceStore.isDeviceConnected}
					Change Device...
				{:else}
					Connect Device...
				{/if}
			</Menubar.Item>
			<Menubar.Item onSelect={handleOpenDeviceSettings} disabled={!deviceStore.isDeviceConnected}>
				RPC Settings...
			</Menubar.Item>
		</Menubar.Content>
	</Menubar.Menu>
</Menubar.Root>