import { deviceState } from '$lib/states/deviceState.svelte';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import type { RowSelectionState } from '@tanstack/table-core';


export type ChartLayout = 'carousel' | 'vertical' | 'horizontal';
export type StreamLayout = 'grouped' | 'vertical' | 'horizontal';

export interface PlotSeries {
	dataKey: DataColumnId; 
	uPlotSeries: uPlot.Series;
}

class DataColumnStyler {
	#styles = new Map<string, { color: string }>();
	#colors = [
		'#3498db', '#e74c3c', '#2ecc71', '#f1c40f',
		'#9b59b6', '#1abc9c', '#d35400', '#34495e',
		'#e67e22', '#16a085', '#c0392b', '#8e44ad'
	];
	#colorIndex = 0;

	#getNextColor(): string {
		const color = this.#colors[this.#colorIndex % this.#colors.length];
		this.#colorIndex++;
		return color;
	}
	getStyle(dataKey: DataColumnId): { color: string } {
		const key = JSON.stringify(dataKey);
		if (!this.#styles.has(key)) {
			this.#styles.set(key, { color: this.#getNextColor() });
		}
		return this.#styles.get(key)!;
	}
}

export const dataColumnStyler = new DataColumnStyler();

export class PlotConfig {
    id = crypto.randomUUID();
    title = $state('New Plot');
    rowSelection = $state<RowSelectionState>({});
    series = $derived.by((): PlotSeries[] => {
        const newSeries: PlotSeries[] = [];
        const selectedKeys = Object.keys(this.rowSelection);
        for (const keyString of selectedKeys) {
            if (keyString.startsWith('{') && keyString.endsWith('}')) {
                try {
                    const dataKey: DataColumnId = JSON.parse(keyString);
                    
                    const device = deviceState.getDevice(dataKey.port_url, dataKey.device_route);
                    const stream = device?.streams.find(s => s.meta.stream_id === dataKey.stream_id);
                    const column = stream?.columns.find(c => c.index === dataKey.column_index);

                    if (device && stream && column) {
                        const style = dataColumnStyler.getStyle(dataKey);
                        newSeries.push({
                            dataKey: dataKey,
                            uPlotSeries: {
                                label: column.name,
                                stroke: style.color,
                                scale: column.units,
                            }
                        });
                    }
                } catch (e) {
                    console.error("This should not happen. Failed to parse a key that looked like JSON:", keyString, e);
                }
            }
        }
        return newSeries;
    });
    hasData = $state(false);
    viewType = $state<'timeseries' | 'fft'>('timeseries');

    uPlotOptions = $derived.by((): uPlot.Options => {
        if (this.series.length === 0) {
            return {
                width: 800,
                height: 400,
                series: [{}],
                axes: [{}, { show: false }],
            };
        }

        const uniqueUnits = new Set(this.series.map(s => s.uPlotSeries.scale));

        const scalesConfig: Record<string, uPlot.Scale> = {};
        for (const unit of uniqueUnits) {
            if(unit) {
                scalesConfig[unit] = { auto: true };
            }
        }

        const axesConfig: uPlot.Axis[] = [{}]; 

        let yAxisCount = 0;
            
        for (const unit of uniqueUnits) {
            if (!unit) continue;
            const axisOptions: uPlot.Axis = {
                    scale: unit,      // Tie this axis to its scale
                    label: unit,      // Label the axis with the unit name
                    labelGap: 5,
                    stroke: this.series.find(s => s.uPlotSeries.scale === unit)?.uPlotSeries.stroke,
                    values: (u, vals) => {
                        const scale = u.scales[unit];

                        if (!scale || scale.min == null || scale.max == null) {
                            return vals.map(v => v.toFixed(2) + " ");
                        }

                        const range = scale.max - scale.min;
                        
                        let decimals;
                        if (range <= 0) decimals = 2;
                        else if (range < 1) decimals = 3;
                        else if (range < 100) decimals = 2;
                        else decimals = 0;
                        
                        return vals.map(v => v.toFixed(decimals) + " ");
                    },
                };
            if (yAxisCount > 0) {
                axisOptions.side = 1;
                axisOptions.grid = { show: false }; 
            }

            axesConfig.push(axisOptions);
            yAxisCount++;
        }

        const uplotSeriesConfig: uPlot.Series[] = [
            {}, // x-axis placeholder
            ...this.series.map(s => s.uPlotSeries)
        ];

        return {
            width: 800,
            height: 400,
            series: uplotSeriesConfig,
            scales: scalesConfig,
            axes: axesConfig,
            pxAlign: 0,
            legend: { show: true },
            cursor: {
                drag: { setScale: false },
                dataIdx: (self, seriesIdx, hoveredIdx, cursorXVal) => {
                    if (seriesIdx === 0) {
                        return hoveredIdx;
                    }

                    const yValues = self.data[seriesIdx];
                    const xValues = self.data[0];

                    if (yValues[hoveredIdx] == null) {
							let nonNullLft = null,
								nonNullRgt = null,
								i;

							i = hoveredIdx;
							while (nonNullLft == null && i-- > 0) {
								if (yValues[i] != null)
									nonNullLft = i;
							}

							i = hoveredIdx;
							while (nonNullRgt == null && i++ < yValues.length) {
								if (yValues[i] != null)
									nonNullRgt = i;
							}

							let rgtVal = nonNullRgt == null ?  Infinity : xValues[nonNullRgt];
							let lftVal = nonNullLft == null ? -Infinity : xValues[nonNullLft];

							let lftDelta = cursorXVal - lftVal;
							let rgtDelta = rgtVal - cursorXVal;

							const newIndex = lftDelta <= rgtDelta ? nonNullLft : nonNullRgt;

                            return newIndex ?? hoveredIdx;
                        }

						return hoveredIdx;
                },
                show: true
            },
        };
    });

    constructor(title: string, initialSelection: RowSelectionState = {}) {
        this.title = title;
        this.rowSelection = initialSelection;
    }
}

const DEFAULT_PLOT_HEIGHT = 400; 

class ChartState {
    plots = $state<PlotConfig[]>([]);
    layout = $state<Record<string, number>>({}); 
    chartLayout = $state<ChartLayout>('vertical');

     addPlot() {
        const newPlot = new PlotConfig('New Plot', {});
        
        this.plots.push(newPlot);
        this.layout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
    }
    
    removePlot(plotId: string) {
        const index = this.plots.findIndex(p => p.id === plotId);

        if (index > -1) {
            this.plots.splice(index, 1);
            delete this.layout[plotId];
        }
    }
}

export const chartState = new ChartState();