<script lang="ts">
	import UplotChart from '$lib/components/UplotChart.svelte';
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { invoke } from '@tauri-apps/api/core';
	import type uPlot from 'uplot';

	// --- Configuration ---
	const viewDurationSeconds = 60;

	// --- State Management (Svelte 5 Runes) ---
	let chart = $state<uPlot | undefined>(undefined);
	let data = $state<[number[], number[], number[]]>([[], [], []]);
	let isAutoScrolling = $state(true);

	// --- Data Types from Rust ---
	interface PlotDataPoint {
		timestamp: number;
		values: {
			'imu.accel.x'?: number;
			'imu.accel.y'?: number;
		};
	}

	// --- Chart Options ---
	const options: uPlot.Options = {
		title: 'Live Sensor Data',
		width: 800,
		height: 600,
		scales: {
			'x': { time: true },
		},
		series: [
			{},
			{ label: 'Accel X', stroke: 'red', width: 1 },
			{ label: 'Accel Y', stroke: 'blue', width: 1 },
		],
		hooks: {
			setSelect: [
				(self) => {
					console.log("User zoomed. Disabling auto-scroll.");
					isAutoScrolling = false;
				}
			]
		}
	};

	// --- NEW: Function to invoke the backend command ---
	async function addData() {
		try {
			await invoke('add_dummy_data');
		} catch (e) {
			console.error("Failed to add data:", e);
		}
	}

	async function refreshPlot() {
		try {
			const rawData = await invoke<PlotDataPoint[]>('get_plot_data');

			if (!rawData || rawData.length === 0) {
				return;
			}

			const timestamps: number[] = [];
			const series1: number[] = [];
			const series2: number[] = [];

			for (const point of rawData) {
				timestamps.push(point.timestamp);
				series1.push(point.values['imu.accel.x'] ?? 0);
				series2.push(point.values['imu.accel.y'] ?? 0);
			}

			data = [timestamps, series1, series2];

			if (isAutoScrolling && chart) {
				const latestTimestamp = timestamps[timestamps.length - 1];
				const newMax = latestTimestamp;
				const newMin = newMax - viewDurationSeconds;
				chart.setScale('x', { min: newMin, max: newMax });
			}

		} catch (e) {
			console.error('Failed to get or process plot data:', e);
		}
	}

	function resetView() {
		isAutoScrolling = true;
		refreshPlot();
	}

	onMount(() => {
		let unlistenFn: (() => void) | undefined = undefined;

		const setupListener = async () => {
			// Perform an initial fetch to populate the chart
			await refreshPlot();

			// Listen for the event from the Rust backend
			unlistenFn = await listen('new-data-available', (event) => {
				// Use requestAnimationFrame to prevent layout thrashing
				window.requestAnimationFrame(refreshPlot);
			});
		};

		setupListener();

		return () => {
			if (unlistenFn) {
				unlistenFn();
			}
		};
	});
</script>

<main>
	<h1>Welcome to Trendline-NG</h1>

	<div class="controls">
        <button onclick={addData}>
            Add Dummy Data Point
        </button>

		<button onclick={() => isAutoScrolling = !isAutoScrolling}>
			Toggle Auto-Scroll ({isAutoScrolling ? 'On' : 'Off'})
		</button>
		<button onclick={resetView} disabled={isAutoScrolling}>
			Go to Live View
		</button>
		<span class="status">
			Auto-scroll is <strong>{isAutoScrolling ? 'ACTIVE' : 'PAUSED'}</strong>.
			{ !isAutoScrolling ? 'You can now zoom and pan freely.' : ''}
		</span>
	</div>

	<div class="chart-container">
		{#if data[0].length > 0}
			<UplotChart {options} {data} bind:chart={chart} />
		{:else}
			<div class="placeholder">
				<p>Waiting for data from device...</p>
                <p>(Click "Add Dummy Data Point" to begin)</p>
			</div>
		{/if}
	</div>
</main>

<style>
	main {
		display: flex;
		flex-direction: column;
		align-items: center;
	}
	.controls {
		padding: 1rem;
		margin-bottom: 1rem;
		border: 1px solid #444;
		border-radius: 8px;
		display: flex;
		gap: 1rem;
		align-items: center;
        flex-wrap: wrap;
        justify-content: center;
	}
	.status {
		font-family: monospace;
		font-size: 0.9em;
	}
	.chart-container {
		border: 1px solid #ccc;
		border-radius: 5px;
		padding: 10px;
	}
	.placeholder {
		width: 800px;
		height: 600px;
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		color: #888;
		border: 2px dashed #444;
		border-radius: 5px;
	}
</style>