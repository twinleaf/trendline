<script lang="ts">
    import { deviceStore } from "$lib/stores/device.store.svelte";

	const selectedDeviceDetails = $derived(
		deviceStore.selectedPorts.map((port) => {
			const parentDevice = deviceStore.allDevices.find((d) => d.url === port.url);
			if (!parentDevice) return { name: 'Unknown Device', route: port.id, key: port.id };

			// Check if the selection is the parent device itself
			if (parentDevice.route === port.id) {
				return { name: parentDevice.name, route: parentDevice.route, key: parentDevice.route };
			}

			// Otherwise, find the child stream
			const childDevice = parentDevice.children?.find((c) => c.route === port.id);
			return {
				name: childDevice?.name ?? 'Unknown Stream',
				route: childDevice?.route ?? port.id,
				key: `${parentDevice.url}-${childDevice?.route ?? port.id}`
			};
		})
	);
</script>

<div class="w-full space-y-4">
	<h3 class="text-lg font-semibold">Selected Streams</h3>
	{#if selectedDeviceDetails.length > 0}
		<ul class="space-y-2">
			{#each selectedDeviceDetails as detail (detail.key)}
				<li class="rounded-md border bg-muted/50 p-2 text-sm">
					<p class="font-medium">{detail.name}</p>
					<p class="text-xs text-muted-foreground">Route: {detail.route}</p>
				</li>
			{/each}
		</ul>
	{:else}
		<p class="text-sm text-muted-foreground">No streams selected.</p>
	{/if}
</div>