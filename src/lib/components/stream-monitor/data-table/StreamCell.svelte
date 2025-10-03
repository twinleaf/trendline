<script lang="ts">
	import type { DataColumnId } from '$lib/bindings/DataColumnId';
	import { chartState } from '$lib/states/chartState.svelte';
	import { streamMonitorState } from '$lib/states/streamMonitorState.svelte';
	import type { ColumnStatistics } from '$lib/bindings/ColumnStatistics';
	import * as Collapsible from '$lib/components/ui/collapsible/index.js';
	import { ChevronDown, Eraser, ArrowLeftRight } from '@lucide/svelte';
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
	let displayedValue = $state<number | undefined>(undefined);
	let addButtonEl: HTMLButtonElement;
	let toggleButtonEl: any;

	const BIG_TEXT_UPDATE_MS = 100;
	let _lastDisplayUpdate = 0;

	// --- INCOMING DATA FROM GLOBAL STORE ---
	const key = $derived(JSON.stringify(dataKey));
	const incomingStats = $derived(streamMonitorState.statisticsData.get(key));

	// --- LOCAL DISPLAY STATE (The Snapshot) ---
	let displayStats = $state<ColumnStatistics | null | undefined>(undefined);
	let showHealth = $state(false);

	// --- EFFECTS ---

	$effect(() => {
		if (chartState.isPaused) {
			return;
		}
		displayStats = incomingStats;
	});


	$effect(() => {
		const stats = displayStats;
		if (!stats) {
			displayedValue = undefined;
			_lastDisplayUpdate = 0;
			return;
		}
		const now = performance.now();
		if (displayedValue === undefined || now - _lastDisplayUpdate >= BIG_TEXT_UPDATE_MS) {
			displayedValue = stats.latest_value;
			_lastDisplayUpdate = now;
		}
	});

	const HOLD_MS = 800;   // long-press threshold
	const TAP_MS  = 180;   // max duration for a true tap

	let holdProgress = $state(0);   // 0..1 for the fill
	let holdRaf: number | null = null;
	let holdStart = 0;
	let holding = false;

	function cancelHoldLoop() {
		if (holdRaf !== null) {
			cancelAnimationFrame(holdRaf);
			holdRaf = null;
		}
	}

	function startHold(e: PointerEvent) {
		if (!displayStats || displayStats.persistent.count === 0n) return;

		holding = true;
		holdStart = performance.now();
		cancelHoldLoop();

		const loop = (now: number) => {
			const elapsed = now - holdStart;
			const p = Math.min(elapsed / HOLD_MS, 1);
			holdProgress = p;

			if (p < 1) {
			holdRaf = requestAnimationFrame(loop);
			} else {
			cancelHoldLoop();
			}
		};

		holdRaf = requestAnimationFrame(loop);
	}

	function endHold() {
		if (!holding) return;

		holding = false;
		cancelHoldLoop();

		const elapsed = performance.now() - holdStart;
		const isTap  = elapsed <= TAP_MS;
		const isLong = elapsed >= HOLD_MS;

		if (isLong) {
			streamMonitorState.resetStatisticsPipeline(dataKey, true);
		} else if (isTap) {
			streamMonitorState.resetStatisticsPipeline(dataKey, false);
		}

		holdProgress = 0;
		}

		function abortHold() {
		if (!holding) return;
		holding = false;
		cancelHoldLoop();
		holdProgress = 0;
	}


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
				{#if displayedValue !== undefined}
					<div class="flex flex-col items-end justify-center font-mono leading-none">
						<span class="text-3xl font-semibold tracking-tight">{format(displayedValue)}</span>
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
		<div class="relative overflow-hidden">
			<div class="pt-2 pb-1 relative z-10">
				<div class="w-full rounded-md border p-2 text-sm relative overflow-hidden">
					<!-- svelte-ignore element_invalid_self_closing_tag -->
					<div
						class="absolute inset-x-0 bottom-0 bg-red-500/30 pointer-events-none z-0"
						style={`height: ${Math.round(holdProgress * 100)}%`}
					/>
					<div class="grid grid-cols-[max-content,1fr,1fr] gap-x-4 gap-y-1 font-mono">
						<div class="col-span-1 font-semibold">Metric</div>
						<div class="text-right font-semibold">Window</div>
						<div class="flex items-center justify-end gap-1 text-right font-semibold">
								<span class="pr-1">Persistent</span>

								<Button
									variant="ghost"
									size="icon"
									class="h-6 w-6"
									aria-label={showHealth ? 'Show value metrics' : 'Show health metrics'}
									onclick={() => (showHealth = !showHealth)}
								>
									<ArrowLeftRight class="size-3.5" />
								</Button>

								<Button
									variant="ghost"
									size="icon"
									class="h-6 w-6"
									aria-label="Wipe persistent statistics"
									disabled={!displayStats || displayStats.persistent.count === 0n}
									onpointerdown={startHold}
									onpointerup={endHold}
									onpointerleave={abortHold}
									onpointercancel={abortHold}
								>
									<Eraser class="size-3.5" />
								</Button>
						</div>
						<div class="my-1 col-span-3 border-b"></div>
						{#if showHealth}
								<!-- HEALTH VIEW -->
								<div class="text-muted-foreground">NaNs</div>
								<div class="text-right">{displayStats.window_health.nan_count.toString()}</div>
								<div class="text-right">{displayStats.persistent_health.nan_count.toString()}</div>

								<div class="text-muted-foreground">Gap Count</div>
								<div class="text-right">{displayStats.window_health.gap_count.toString()}</div>
								<div class="text-right">{displayStats.persistent_health.gap_count.toString()}</div>

								<div class="text-muted-foreground">Gap Mean</div>
								<div class="text-right">{format(displayStats.window_health.gap_mean)}</div>
								<div class="text-right">{format(displayStats.persistent_health.gap_mean)}</div>

								<div class="text-muted-foreground">Gap Min</div>
								<div class="text-right">{format(displayStats.window_health.gap_min)}</div>
								<div class="text-right">{format(displayStats.persistent_health.gap_min)}</div>

								<div class="text-muted-foreground">Gap Max</div>
								<div class="text-right">{format(displayStats.window_health.gap_max)}</div>
								<div class="text-right">{format(displayStats.persistent_health.gap_max)}</div>
							{:else}
								<!-- VALUE VIEW -->
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
						{/if}
					</div>
				</div>
			</div>
		</div>
		{/if}
	</Collapsible.Content>
</Collapsible.Root>