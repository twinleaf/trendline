<script lang="ts">
	import { deviceState } from '$lib/states/deviceState.svelte';
	import type { UiDevice } from '$lib/bindings/UiDevice';
	let selected: UiDevice[] = $derived(deviceState.selectedDevices);
</script>

<div class="w-full space-y-4">
	<h3 class="text-lg font-semibold">Selected Streams</h3>

	{#if selected.length}
		<ul class="space-y-2">
			{#each selected as device (device.url + device.route)}
				<li class="rounded-md border bg-muted/50 p-2 text-sm">
					<p class="font-medium">{device.meta.name}</p>
					<p class="text-xs text-muted-foreground">Route: {device.route}</p>
				</li>
			{/each}
		</ul>
	{:else}
		<p class="text-sm text-muted-foreground">No streams selected.</p>
	{/if}
</div>