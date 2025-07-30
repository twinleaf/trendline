<script lang="ts">
	import type { PlotSeries } from '$lib/states/chartState.svelte';

	let {
		viewType,
		relativeTime,
		frequency,
		series,
		values,
		isActive,
		cursorLeft,
		cursorTop,
		chartBounds
	} = $props<{
		viewType: 'timeseries' | 'fft';
		relativeTime: number | null;
		frequency: number | null;
		series: PlotSeries[];
		values: (number | null)[];
		isActive: boolean;
		cursorLeft: number | null;
		cursorTop: number | null;
		chartBounds: DOMRect;
	}>();
    // svelte-ignore non_reactive_update
	let legendEl: HTMLDivElement;
	let style = $state('');

	const headerText = $derived.by(() => {
		if (viewType === 'fft') {
			return frequency === null ? '---' : `${frequency.toFixed(2)} Hz`;
		}
		return relativeTime === null ? '---' : `${relativeTime.toFixed(3)}s`;
	});

	function formatValue(value: number | null, vType: 'timeseries' | 'fft'): string {
		if (value === null || !isFinite(value)) return '---';
		if (vType === 'fft') return value.toExponential(2);
		return value.toExponential(3);
	}

	$effect(() => {
		if (isActive && cursorLeft != null && cursorTop != null && legendEl) {
			const legendRect = legendEl.getBoundingClientRect();
			if (legendRect.width === 0) return;

			const margin = 5;
            const cursorOffset = 150;
			const anchor = {
				left: chartBounds.left + cursorLeft,
				top: chartBounds.top + cursorTop
			};

			let top = anchor.top - legendRect.height / 2;
			top = Math.max(chartBounds.top, top);
			top = Math.min(top, chartBounds.bottom - legendRect.height);

			const requiredWidth = legendRect.width;
			const spaceOnLeft = anchor.left - chartBounds.left - margin;
			const spaceOnRight = chartBounds.right - anchor.left - margin;
			let left;

			if (spaceOnLeft >= requiredWidth) {
				left = anchor.left - requiredWidth - margin;
			}
			else if (spaceOnRight >= requiredWidth) {
				left = anchor.left + cursorOffset;
			}
			else {
                left =
                    spaceOnRight > spaceOnLeft
                        ? anchor.left + cursorOffset
                        : anchor.left - requiredWidth - margin;
            }

			style = `top: ${top}px; left: ${left}px;`;
		}
	});
</script>

{#if isActive}
	<div bind:this={legendEl} class="custom-legend" style="position: fixed; {style}">
		<div class="timestamp">{headerText}</div>
		<div class="series-values">
			{#each series as s, i (s.dataKey)}
				{#if values[i] !== undefined && values[i] !== null}
					<div class="series-item">
						<span class="color-dot" style:background-color={s.uPlotSeries.stroke}></span>
						<span class="label">{s.uPlotSeries.label}:</span>
						<span class="value">{formatValue(values[i], viewType)}</span>
					</div>
				{/if}
			{/each}
		</div>
	</div>
{/if}

<style>
	.custom-legend {
		z-index: 100;
		background-color: rgba(var(--background), 0.85);
		border-radius: 4px;
		padding: 8px;
		font-family: sans-serif;
		font-size: 12px;
		border: 1px solid var(--border);
		white-space: nowrap;
		pointer-events: none;
		backdrop-filter: blur(2px);
	}

	.timestamp {
		font-weight: bold;
		margin-bottom: 5px;
	}
	.series-item {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.color-dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		display: inline-block;
	}
	.value {
		font-weight: 500;
	}
</style>