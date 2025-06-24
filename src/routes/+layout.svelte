<script lang="ts">
    import 'uplot/dist/uPlot.min.css';
    import "../app.css";
    import MenubarHeader from '$lib/components/MenubarHeader.svelte';
	import StatusFooter from '$lib/components/StatusFooter.svelte';
	import DeviceSelector from '$lib/components/DeviceSelector.svelte';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';

    import { uiStore } from '$lib/stores/ui.store.svelte';
	import { deviceStore } from '$lib/stores/device.store.svelte';
    import { chartStore } from '$lib/stores/chart.store.svelte';
    import type { FeDeviceMeta } from '$lib/bindings/FeDeviceMeta';
	import type { PortInfo } from '$lib/bindings/PortInfo';
    import type { Snippet } from 'svelte'; 

	import { onMount } from 'svelte';

    let { children } = $props<{ children: Snippet }>();

    $effect(() => {
		const { selectedPorts, allDevices } = deviceStore;

		if (selectedPorts.length > 0 && allDevices.length > 0) {
			console.log('Layout Effect: Device changed, initializing charts...');
			chartStore.initializeCharts(selectedPorts, allDevices);
		} else {
			console.log('Layout Effect: No device, clearing charts...');
			chartStore.charts = []; 
		}
	});

    onMount(() => {
		if (!deviceStore.isDeviceConnected) {
			uiStore.openDiscoveryDialog();
		}
	});

    function handleDeviceSelection(ports: PortInfo[], devices: FeDeviceMeta[]) {
		deviceStore.startStreaming(ports, devices);
		uiStore.closeDiscoveryDialog();
	}
</script>

<div class="flex h-screen flex-col gap-1 p-4">
	<header>
		<MenubarHeader />
	</header>

	<main class="flex-grow overflow-auto">
		<div class="h-full rounded-lg border bg-card p-1 shadow-sm">
			{@render children()}
		</div>
	</main>

	<footer>
		<StatusFooter />
	</footer>

	<AlertDialog.Root bind:open={uiStore.discoveryDialogOpen}>
		<AlertDialog.Content
			class="flex w-full max-w-md flex-col gap-4 bg-card p-6"
			onEscapeKeydown={uiStore.closeDiscoveryDialog}
			onInteractOutside={uiStore.closeDiscoveryDialog}
		>
			<DeviceSelector onConfirm={handleDeviceSelection} />
		</AlertDialog.Content>
	</AlertDialog.Root>
</div>