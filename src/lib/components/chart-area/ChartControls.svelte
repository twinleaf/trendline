<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { LoaderCircleIcon, PlayIcon, SquareIcon } from '@lucide/svelte';
	import type { OperatingMode } from '$lib/stores/chart.store.svelte';

	let {
		onToggleRunStop,
		mode,
		isLoading
	} = $props<{
		onToggleRunStop: () => void;
		mode: OperatingMode;
		isLoading: boolean;
	}>();
</script>

<div class="controls">
	<Button onclick={onToggleRunStop} class="w-32">
		{#if mode === 'live'}
			<SquareIcon class="mr-2 h-4 w-4" />
			Stop
		{:else}
			<PlayIcon class="mr-2 h-4 w-4" />
			Run
		{/if}
	</Button>

	{#if isLoading && mode === 'stopped'}
		<div class="flex items-center text-sm text-muted-foreground">
			<LoaderCircleIcon class="mr-2 h-4 w-4 animate-spin" />
			<span>Fetching...</span>
		</div>
	{/if}
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