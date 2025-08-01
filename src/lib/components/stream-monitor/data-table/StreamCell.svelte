<script lang="ts">
	import type { DataColumnId } from '$lib/bindings/DataColumnId';
	import { chartState } from '$lib/states/chartState.svelte';
	import { streamMonitorState } from '$lib/states/streamMonitorState.svelte';
	import type { StreamStatistics } from '$lib/bindings/StreamStatistics'; // Import the type
	import * as Collapsible from '$lib/components/ui/collapsible/index.js';
	import { ChevronDown, Eraser } from '@lucide/svelte';
	import NameCell from './NameCell.svelte';
	import { Button } from '$lib/components/ui/button';

	type Props = {
		dataKey: DataColumnId;
		name: string;
		units?: string;
		depth?: number;
	};
	let { dataKey, name, units }: Props = $props();

	// --- LOCAL UI STATE ---
	let smoothedValue = $state<number | undefined>(undefined);
	let addButtonEl: HTMLButtonElement;
	let toggleButtonEl: any;

	const SMOOTHING_FACTOR = 0.1;

	// --- INCOMING DATA FROM GLOBAL STORE ---
	const key = $derived(JSON.stringify(dataKey));
	const incomingStats = $derived(streamMonitorState.statisticsData.get(key));

	// --- LOCAL DISPLAY STATE (The Snapshot) ---
	let displayStats = $state<StreamStatistics | null | undefined>(undefined);

	// --- EFFECTS ---

	$effect(() => {
		if (chartState.isPaused) {
			return;
		}
		displayStats = incomingStats;
	});


	$effect(() => {
		const currentDisplayStats = displayStats;
		if (currentDisplayStats) {
			const latestValue = currentDisplayStats.latest_value;
			if (smoothedValue === undefined) {
				smoothedValue = latestValue;
			} else {
				smoothedValue = SMOOTHING_FACTOR * latestValue + (1 - SMOOTHING_FACTOR) * smoothedValue;
			}
		} else {
			smoothedValue = undefined;
		}
	});

	// --- HELPER FUNCTIONS ---
	function format(value: number | undefined): string {
		if (value === undefined || !isFinite(value)) return '---';
		const absValue = Math.abs(value);
		if (absValue === 0) return '0.000';
		if (absValue < 0.01 || absValue >= 1000) return value.toExponential(2);
		return value.toFixed(3);
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === ' ') {
			event.preventDefault();
			return;
		}
		if (event.key === 'ArrowRight') {
			if (document.activeElement === addButtonEl) {
				event.preventDefault();
				toggleButtonEl?.focus();
			}
		} else if (event.key === 'ArrowLeft') {
			if (document.activeElement === toggleButtonEl) {
				event.preventDefault();
				addButtonEl?.focus();
			}
		}
	}

	async function resetPersistentStats() {
		await streamMonitorState.resetStatistics(dataKey);
	}
</script>

<Collapsible.Root class="group w-full">
	<div class="flex flex-col">
		<div class="pl-1 pb-1 text-sm text-muted-foreground">
			<NameCell {name} depth={0} />
		</div>
		<div
			class="flex h-16 w-full items-stretch overflow-hidden rounded-md border bg-background focus-within:ring-2 focus-within:ring-ring focus-within:ring-offset-2"
		>
			<button
				bind:this={addButtonEl}
				class="flex flex-grow items-center justify-center p-2 text-left hover:bg-muted focus:outline-none"
				aria-label={`Add plot for ${name}`}
				onclick={() => chartState.addPlotFromStream(dataKey, name)}
				onkeydown={handleKeydown}
			>
				{#if smoothedValue !== undefined}
					<div class="flex flex-col items-end justify-center font-mono leading-none">
						<span class="text-3xl font-semibold tracking-tight">{format(smoothedValue)}</span>
						<span class="text-xs text-muted-foreground">{units}</span>
					</div>
				{:else}
					<span class="font-mono text-muted-foreground">---</span>
				{/if}
			</button>
			<div class="w-px bg-border"></div>
			<Collapsible.Trigger
				bind:this={toggleButtonEl}
				disabled={!displayStats}
				aria-label="Toggle details"
				class="flex basis-12 items-center justify-center hover:bg-muted focus:outline-none data-[state=open]:bg-muted"
				tabindex={-1}
				onkeydown={handleKeydown}
			>
				<ChevronDown class="h-4 w-4 transition-transform group-data-[state=open]:rotate-180" />
			</Collapsible.Trigger>
		</div>
	</div>
	<Collapsible.Content
		class="w-full overflow-hidden transition-all data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down"
	>
		{#if displayStats}
			<div class="pt-2 pb-1">
				<div class="w-full rounded-md border p-2 text-sm">
					<div class="grid grid-cols-[max-content,1fr,1fr] gap-x-4 gap-y-1 font-mono">
						<div class="col-span-1 font-semibold">Metric</div>
						<div class="text-right font-semibold">Window</div>
						<div class="flex items-center justify-end gap-2 text-right font-semibold">
							<span>Persistent</span>
							<Button
								variant="ghost"
								size="icon"
								class="h-6 w-6"
								aria-label="Wipe persistent statistics for {name}"
								disabled={!displayStats || displayStats.persistent.count === 0n}
								onclick={resetPersistentStats}
							>
								<Eraser class="size-3.5" />
							</Button>
						</div>
						<div class="my-1 col-span-3 border-b"></div>
						<div class="text-muted-foreground">Mean</div>
						<div class="text-right">{format(displayStats.window.mean)}</div>
						<div class="text-right">{format(displayStats.persistent.mean)}</div>
						<div class="text-muted-foreground">StdDev</div>
						<div class="text-right">{format(displayStats.window.stdev)}</div>
						<div class="text-right">{format(displayStats.persistent.stdev)}</div>
						<div class="text-muted-foreground">Min</div>
						<div class="text-right">{format(displayStats.window.min)}</div>
						<div class="text-right">{format(displayStats.persistent.min)}</div>
						<div class="text-muted-foreground">Max</div>
						<div class="text-right">{format(displayStats.window.max)}</div>
						<div class="text-right">{format(displayStats.persistent.max)}</div>
						<div class="text-muted-foreground">RMS</div>
						<div class="text-right">{format(displayStats.window.rms)}</div>
						<div class="text-right">{format(displayStats.persistent.rms)}</div>
						<div class="text-muted-foreground">Count</div>
						<div class="text-right">{displayStats.window.count.toString()}</div>
						<div class="text-right">{displayStats.persistent.count.toString()}</div>
					</div>
				</div>
			</div>
		{/if}
	</Collapsible.Content>
</Collapsible.Root>