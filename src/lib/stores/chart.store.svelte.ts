// src/lib/stores/chart.store.svelte.ts

import uPlot from 'uplot';
import { invoke } from '@tauri-apps/api/core';
import type { FeDeviceMeta } from '$lib/bindings/FeDeviceMeta';
import type { PortInfo } from '$lib/bindings/PortInfo';
import type { SinglePlotPoint } from '$lib/bindings/SinglePlotPoint';

// Type definitions remain the same
interface ChartState {
	id: string;
	options: uPlot.Options;
	data: uPlot.AlignedData;
	seriesKeys: string[];
	seriesKeyToIndex: Map<string, number>;
	startTime: number | null;
}

class ChartStore {
	charts = $state<ChartState[]>([]);
	isFetchingAll = $state(false);

	initializeCharts(ports: PortInfo[], devices: FeDeviceMeta[]) {
		const portsByRoute = new Map<string, PortInfo[]>();
		for (const port of ports) {
			if (!portsByRoute.has(port.id)) {
				portsByRoute.set(port.id, []);
			}
			portsByRoute.get(port.id)!.push(port);
		}

		const newCharts: ChartState[] = [];
		const chartColors = [
			'#7cb5ec', '#434348', '#90ed7d', '#f7a35c', '#8085e9',
			'#f15c80', '#e4d354', '#2b908f', '#f45b5b', '#91e8e1'
		];

		for (const [route, portsInRoute] of portsByRoute.entries()) {
			let colorIndex = 0;
			const uplotSeries: uPlot.Series[] = [
                {
                    label: 'Time (s)',
                    value: (u, v) => v == null ? '–' : `${v.toFixed(3)}s`
                }
            ];
			const localSeriesKeys: string[] = [];
			const localSeriesKeyToIndex = new Map<string, number>();
			const axes: uPlot.Axis[] = [
                { 
                    values: (self, ticks) => ticks.map(v => `${v.toFixed(1)}s`)
                },
                {
                    label: "Value",
                    scale: 'y_default',
                    grid: { show: true }, 
                }
            ];
			const scaleUnits = new Map<string, string>();
	
			for (const port of portsInRoute) {
				const parentDevice = devices.find((d) => d.url === port.url);
				if (!parentDevice) continue;
	
				const deviceMeta =
					port.id === parentDevice.route
						? parentDevice
						: parentDevice.children?.find((c) => c.route === port.id);
				if (!deviceMeta) continue;
	
				for (const stream of deviceMeta.streams || []) {
					for (const column of stream.columns || []) {
						if (column.name.toLowerCase() === 'timestamp') continue;
                        console.log(`[Chart ${route}] Adding series for column: ${column.name}`);

						const key = `${route}-${stream.name}-${column.name}`;
						localSeriesKeys.push(key);
						localSeriesKeyToIndex.set(key, uplotSeries.length);
	
						const unit = column.units || 'value';
						const scaleId = `y_${unit.replace(/[^a-zA-Z0-9]/g, '')}`;
	
						if (!scaleUnits.has(scaleId)) {
							scaleUnits.set(scaleId, unit);
							axes.push({
								scale: scaleId,
								label: unit,
								side: scaleUnits.size % 2 === 0 ? 1 : 3,
								grid: { show: scaleUnits.size === 1 }
							});
						}
						uplotSeries.push({
							label: `${deviceMeta.name}: ${column.name}`,
							stroke: chartColors[colorIndex++ % chartColors.length],
							scale: scaleId,
							value: (u, v) => (v == null ? '–' : `${v.toFixed(3)} ${column.units}`),
							points: { show: false }
						});
					}
				}
			}

			if (localSeriesKeys.length > 0) {
				newCharts.push({
					id: route,
					seriesKeys: localSeriesKeys,
					seriesKeyToIndex: localSeriesKeyToIndex,
					startTime: null,
					data: [[]],
					options: {
						title: `Device Route: ${route}`,
						width: 800,
						height: 400,
						series: uplotSeries,
						axes,
						scales: Object.fromEntries(
							Array.from(scaleUnits.keys()).map((id) => [id, { auto: true }])
						),
						cursor: { drag: { x: true, y: true, setScale: true } }
					}
				});
			}
		}

		this.charts = newCharts;
	}

	async fetchAllData() {
		const allSeriesKeys = this.charts.flatMap((c) => c.seriesKeys);
		if (allSeriesKeys.length === 0) return;

		this.isFetchingAll = true;
		try {
			const response = await invoke<Record<string, SinglePlotPoint[]>>('get_plot_data_in_range', {
				seriesKeys: allSeriesKeys,
				startTime: 0,
				endTime: Date.now() / 1000 + 10
			});

			// 1. Create a reverse map from a series key to its chart details.
			const seriesToChartMap = new Map<string, { chartId: string; seriesIndex: number }>();
			for (const chart of this.charts) {
				for (const [key, index] of chart.seriesKeyToIndex.entries()) {
					seriesToChartMap.set(key, { chartId: chart.id, seriesIndex: index });
				}
			}

			// 2. Prepare a map to hold points partitioned by chart ID.
			const pointsByChart = new Map<string, { x: number; y: number; seriesIndex: number }[]>();
			for (const chart of this.charts) {
				pointsByChart.set(chart.id, []);
			}

			// 3. Iterate through the response ONCE and distribute points efficiently.
			for (const [key, points] of Object.entries(response)) {
				const mapping = seriesToChartMap.get(key);
				if (mapping) {
					const targetArray = pointsByChart.get(mapping.chartId);
					if (targetArray) {
						for (const point of points) {
							targetArray.push({ x: point.x, y: point.y, seriesIndex: mapping.seriesIndex });
						}
					}
				}
			}

			for (const chart of this.charts) {
				const allPoints = pointsByChart.get(chart.id)!;
				allPoints.sort((a, b) => a.x - b.x);

				if (allPoints.length === 0) {
					chart.data = [[]]; 
					continue;
				}

				if (chart.startTime === null) {
					chart.startTime = allPoints[0].x;
				}

				// 1. Get all unique, sorted timestamps for this chart's data
				const uniqueTimestamps = [...new Set(allPoints.map((p) => p.x))];

				// 2. Create the x-axis data (relative time)
				const xData = uniqueTimestamps.map((t) => t - chart.startTime!);

				// 3. Create a map for O(1) lookup of a timestamp's index
				const timestampToIndex = new Map(uniqueTimestamps.map((t, i) => [t, i]));

				// 4. Initialize y-series arrays, pre-filled with nulls to match xData length
				const yDataSeries: (number | null)[][] = Array.from(
					{ length: chart.options.series.length - 1 },
					() => Array(xData.length).fill(null)
				);

				// 5. Populate the y-series arrays at the correct indices
				for (const point of allPoints) {
					const xIndex = timestampToIndex.get(point.x);
					const ySeriesIndex = point.seriesIndex - 1; // seriesIndex is 1-based

					if (xIndex !== undefined && ySeriesIndex >= 0 && ySeriesIndex < yDataSeries.length) {
						// Place the y-value in the correct spot. If multiple points have the same
						// timestamp and series, the last one wins (which is fine).
						yDataSeries[ySeriesIndex][xIndex] = point.y;
					}
				}

				// 6. Assign the final, correctly pivoted data to the chart
				chart.data = [xData, ...yDataSeries];
			}
		} catch (e) {
			console.error(`Failed to get plot data:`, e);
		} finally {
			this.isFetchingAll = false;
		}
	}

	clearAllCharts() {
		for (const chart of this.charts) {
			chart.data = [[]];
			chart.startTime = null;
		}
		this.charts = [...this.charts]; // Trigger reactivity for array-level consumers if needed
		console.log('All chart data cleared.');
	}
}

export const chartStore = new ChartStore();