<script lang="ts">
	import UplotSvelte from 'uplot-svelte';
	import type uPlot from 'uplot';
	import SeriesMetadata from './DeviceSelector.svelte'; // Import the type

	let {
		seriesData,
		seriesMetadata
	} = $props<{
		seriesData: Map<string, { x: number; y: number }[]>;
		seriesMetadata: Map<string, SeriesMetadata>;
	}>();

	let options: uPlot.Options | undefined = $state();
	let uplotData: uPlot.AlignedData | undefined = $state();

	const colorPalette = ['#e53935', '#1e88e5', '#43a047', '#fdd835', '#fb8c00', '#8e24aa'];
	const seriesColors = new Map<string, string>();
	let colorIndex = 0;

	function getSeriesColor(seriesName: string): string {
		if (!seriesColors.has(seriesName)) {
			seriesColors.set(seriesName, colorPalette[colorIndex % colorPalette.length]);
			colorIndex++;
		}
		return seriesColors.get(seriesName)!;
	}

	$effect(() => {
		if (seriesData.size === 0) {
			uplotData = undefined;
			options = undefined;
			return;
		}

		// --- Derive Title and Axis Labels from Metadata ---
		let chartTitle = 'Live Sensor Data';
		let yAxisLabel = 'Value';

		const firstMeta = seriesMetadata.values().next().value;
		if (firstMeta) {
			chartTitle = `${firstMeta.deviceName} - ${firstMeta.streamName}`;
			yAxisLabel = firstMeta.units || 'Value'; // Fallback if units are empty
		}
		// ---

		const allTimestamps = new Set<number>();
		for (const points of seriesData.values()) {
			points.forEach(p => allTimestamps.add(p.x));
		}
		const sortedTimestamps = Array.from(allTimestamps).sort((a, b) => a - b);

		const uplotSeries: uPlot.Series[] = [
            { label: 'Time', value: '{YYYY}-{MM}-{DD} {HH}:{mm}:{ss}.{fff}' }
        ];
		const alignedSeriesData: (number | null)[][] = [];

		for (const [key, points] of seriesData.entries()) {
			const pointMap = new Map(points.map(p => [p.x, p.y]));
			alignedSeriesData.push(sortedTimestamps.map(ts => pointMap.get(ts) ?? null));

			uplotSeries.push({
				label: key,
				stroke: getSeriesColor(key),
				width: 2,
				scale: 'y'
			});
		}

		options = {
			title: chartTitle,
			width: 800,
			height: 600,
			series: uplotSeries,
			scales: { 
                y: { auto: true },
                // Use time: true for proper timestamp handling by uPlot
                x: { time: true } 
            },
			axes: [
                { label: 'Time' }, // X-axis
                { label: yAxisLabel, scale: 'y' } // Y-axis
            ]
		};

		// uPlot expects timestamps in seconds, not milliseconds
		uplotData = [sortedTimestamps.map(ts => ts / 1000), ...alignedSeriesData];
	});
</script>

{#if options && uplotData && uplotData[0] && uplotData[0].length > 0}
	<UplotSvelte {options} data={uplotData} />
{:else}
	<div class="placeholder" style="width: 800px; height: 600px;">
		<p>Waiting for data...</p>
	</div>
{/if}

<style>
	.placeholder {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		color: oklch(var(--muted-foreground));
		border: 2px dashed oklch(var(--border));
		border-radius: var(--radius);
	}
</style>