<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { LoaderCircleIcon } from '@lucide/svelte';

	// Simpler props, no defaults needed.
	let {
		onFetchData,
		isLoading
	} = $props<{
		onFetchData: () => void;
		isLoading: boolean;
	}>();
</script>

<div class="controls">
	<Button onclick={onFetchData} disabled={isLoading} class="w-32">
		{#if isLoading}
			<LoaderCircleIcon class="mr-2 h-4 w-4 animate-spin" />
			Fetching...
		{:else}
			Fetch Data
		{/if}
	</Button>
</div>

<style>
	.controls {
		padding: 1rem;
		margin-bottom: 1rem;
		border: 1px solid oklch(var(--border));
		border-radius: var(--radius);
		display: flex;
		gap: 1rem;
		align-items: center;
		flex-wrap: wrap;
		justify-content: center;
	}
</style>