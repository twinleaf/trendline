<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	
	import DeviceSelector from '$lib/components/DeviceSelector.svelte';
	import ChartControls from '$lib/components/ChartControls.svelte';
	import LiveChart from '$lib/components/LiveChart.svelte';

	// --- Import types from generated bindings ---
	import type { PortInfo } from '$lib/bindings/PortInfo';
	import type { SinglePlotPoint } from '$lib/bindings/SinglePlotPoint';
	
	// --- Frontend-specific types can be defined here ---
	interface PlotPoint { x: number; y: number; }
	interface SeriesMetadata {
		units: string;
		deviceName: string;
		streamName: string;
	}

	// --- State ---
	let streamingStarted = $state(false);
	let isAutoScrolling = $state(true);
	let viewStartTime = $state(0);
	let viewEndTime = $state(0);
	
	let seriesData = $state(new Map<string, PlotPoint[]>());
	let seriesMetadata = $state(new Map<string, SeriesMetadata>());

	async function handleStartStreaming(ports: PortInfo[]) {
		console.log('Received start instruction with ports:', ports);
		if (ports.length > 0) {
			try {
				await invoke('start_streaming', { ports });
				streamingStarted = true;
			} catch (e) {
				console.error("Failed to start streaming:", e);
			}
		}
	}

	$effect(() => {
		if (!streamingStarted) return;
		
		console.log("Setting up event listener for new-data-available");
		const unlisten = listen<SinglePlotPoint[]>('new-data-available', (event) => {
			const newPoints = event.payload;
			
			const updatedMap = new Map(seriesData);
			let dataChanged = false;

			for (const point of newPoints) {
				if (!updatedMap.has(point.series_key)) {
					updatedMap.set(point.series_key, []);
				}
				updatedMap.get(point.series_key)!.push({ x: point.x, y: point.y });
				dataChanged = true;
			}
			
			if (dataChanged) {
				seriesData = updatedMap;
			}
		});
		
		return () => {
			console.log("Cleaning up event listener");
			unlisten.then(f => f());
		};
	});

	// Effect for auto-scrolling the chart view
	$effect(() => {
		if (!isAutoScrolling || !streamingStarted) return;

		const updateView = () => {
			const now = Date.now(); // Timestamps for uPlot are best handled in milliseconds
			viewEndTime = now;
			viewStartTime = now - 30_000; // 30-second rolling window
		};
		updateView();
		const interval = setInterval(updateView, 1000);
		
		return () => clearInterval(interval);
	});

</script>

<main class="flex flex-col items-center p-4">
	{#if !streamingStarted}
		<!-- We now pass handleStartStreaming as a prop named onStart -->
		<DeviceSelector onStart={handleStartStreaming} />
	{:else}
		<div class="w-full max-w-4xl space-y-4">
			<h1 class="text-2xl font-bold text-center">Live Data Viewer</h1>
			<ChartControls bind:isAutoScrolling bind:viewStartTime bind:endStartTime={viewEndTime} />
			<div class="rounded-lg border p-2">
				<!-- Passing both data and metadata to the chart -->
				<!-- <LiveChart {seriesData} {seriesMetadata} /> -->
			</div>
		</div>
	{/if}
</main>
