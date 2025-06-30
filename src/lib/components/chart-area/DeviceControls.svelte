<script lang="ts">
	import { deviceState } from '$lib/states/deviceState.svelte';
</script>

<div class="w-full space-y-4">
	<h3 class="text-lg font-semibold">Selected Streams</h3>

	{#if deviceState.selectedDeviceDetails().length}
		<ul class="space-y-2">
			{#each deviceState.selectedDeviceDetails() as detail (detail.key)}
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
