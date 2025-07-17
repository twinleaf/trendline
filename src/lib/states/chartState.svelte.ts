import { deviceState } from '$lib/states/deviceState.svelte';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import type { DecimationMethod } from '$lib/bindings/DecimationMethod';
import type { RowSelectionState } from '@tanstack/table-core';
import type { ExpandedState } from '@tanstack/table-core';


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
    expansion = $state<ExpandedState>({});
    activeTab = $state<'selection' | 'settings'>('selection');
    scrollTops = $state<{ selection: number; settings: number }>({
        selection: 0,
        settings: 0
    });

    #manualDecimationMethod = $state<DecimationMethod>('Fpcs');
    #isDecimationManual = $state(false);
    windowSeconds = $state<number>(30.0);
    fftSeconds = $state<number>(10.0);
    fftYAxisPower = $state(4);

    hasData = $state(false);
    latestTimestamp = $state(0);

    get decimationMethod(): DecimationMethod {
        if (this.#isDecimationManual) {
            return this.#manualDecimationMethod;
        }
        return this.series.length > 1 ? 'MinMax' : 'Fpcs';
    }

    set decimationMethod(value: DecimationMethod) {
        this.#isDecimationManual = true;
        this.#manualDecimationMethod = value;
    }

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

    viewType = $state<'timeseries' | 'fft'>('timeseries');

    maxSamplingRate = $derived.by((): number => {
        if (this.series.length === 0) {
            return 0;
        }
        const rates = this.series.map(s => {
            const device = deviceState.getDevice(s.dataKey.port_url, s.dataKey.device_route);
            const stream = device?.streams.find(st => st.meta.stream_id === s.dataKey.stream_id);
            return stream?.effective_sampling_rate ?? 0;
        });
        return Math.max(...rates);
    });

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
                if (this.viewType === 'fft') {
                    scalesConfig[unit] = { 
                        auto: true,
                        range: (u, dataMin, dataMax) => {
                        const HYSTERESIS_FACTOR = 0.1;

                        if (dataMin <= 0 || dataMax <= 0) {
                            return [10 ** -this.fftYAxisPower, 10 ** this.fftYAxisPower];
                        }

                        const maxMagnitude = Math.max(Math.abs(Math.log10(dataMin)), Math.abs(Math.log10(dataMax)));
                        const currentPower = this.fftYAxisPower;

                        if (maxMagnitude > currentPower) {
                            this.fftYAxisPower = Math.ceil(maxMagnitude);
                        }
                        else if (maxMagnitude < currentPower - 1 - HYSTERESIS_FACTOR) {
                            this.fftYAxisPower = Math.ceil(maxMagnitude);
                        }
                        return [10 ** -this.fftYAxisPower, 10 ** this.fftYAxisPower];
                    }
                    };
                }
                else {
                    scalesConfig[unit] = { auto: true };
                }
            }
        }
        const axesConfig: uPlot.Axis[] = [{}];

        if (this.viewType === 'fft') {
            scalesConfig['x'] = { time: false, distr: 3, log: 10 };
            axesConfig[0] = { 
                scale: 'x',
                label: "Frequency (Hz)",
            };

            for (const unit of uniqueUnits) {
                if (unit && scalesConfig[unit]) {
                    scalesConfig[unit].distr = 3;
                    scalesConfig[unit].log = 10;
                }
            }
        } else {
            scalesConfig['x'] = { time: false };
             axesConfig[0] = {
                scale: 'x',
                space: 100, 
                values: (self, ticks) => {
                    const latest_t = this.latestTimestamp;
                    if (latest_t === 0) return ticks.map(t => t.toFixed(1));

                    return ticks.map(rawTick => {
                        const secondsAgo = latest_t - rawTick;

                        if (Math.abs(secondsAgo) < 0.01) {
                            return "Now";
                        }
                        
                        return `-${secondsAgo.toFixed(1)}s`;
                    });
                }
            };
        }

        let yAxisCount = 0;
            
        for (const unit of uniqueUnits) {
            if (!unit) continue;
            const axisOptions: uPlot.Axis = {
                scale: unit,
                label: unit,
                labelGap: 5,
                stroke: this.series.find(s => s.uPlotSeries.scale === unit)?.uPlotSeries.stroke,
                values: (u, vals) => {
                    if (!vals.length) {
                        return [];
                    }

                     return vals.map(v => {
                        if (v == null) return "";
                        if (v === 0) return "0 "; 

                        const absV = Math.abs(v);

                        if (absV > 0 && absV < 0.01) {
                            return v.toExponential(1) + " ";
                        }
                        if (absV < 10) {
                            return v.toFixed(2) + " ";
                        }
                        if (absV < 100) {
                            return v.toFixed(1) + " ";
                        }
                        return v.toFixed(0) + " ";
                    });
                }
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