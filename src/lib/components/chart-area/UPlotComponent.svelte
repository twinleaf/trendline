<script lang="ts">
    import uPlot from "uplot";
    import "uplot/dist/uPlot.min.css";
    import { untrack } from "svelte";
    import { chartState } from "$lib/states/chartState.svelte";
    import type { PlotConfig, PlotSeries } from "$lib/states/chartState.svelte";
    import CustomLegend from "$lib/components/chart-area/legend/CustomLegend.svelte";
    import { findTimestampIndex, lerp } from "$lib/utils";
    import { mode } from "mode-watcher";
    import type { PlotData } from "$lib/bindings/PlotData";

    // --- Props ---
    let {
        plot,
        latestTimestamp = $bindable(),
    }: { plot: PlotConfig; latestTimestamp?: number } = $props();

    // --- Component State ---
    let chartContainer: HTMLDivElement;
    let uplot = $state.raw<uPlot | undefined>(undefined);
    let resizeObserver: ResizeObserver | undefined;
    let currentFingerprint = "";
    let legendState = $state({
        isActive: false,
        relativeTime: null as number | null,
        frequency: null as number | null,
        values: [] as (number | null)[],
        cursorLeft: null as number | null,
        cursorTop: null as number | null,
        chartBounds: null as DOMRect | null,
    });

    // --- Simple derived state ---
    const isFFT = $derived(plot.viewType === "fft");
    const rawPlotData = $derived(chartState.plotsData.get(plot.id));
    const isDark = $derived((mode.current ?? "light") === "dark");

    // --- Options builder (pure function) ---
    function buildOptions(
        series: PlotSeries[],
        viewType: "timeseries" | "fft",
        dark: boolean,
    ): uPlot.Options {
        const gridColor = dark ? "#2e3a35" : "#d8e1dc";
        const labelColor = dark ? "#d1ddd5" : "#2c3732";

        if (series.length === 0) {
            return {
                width: 800,
                height: 400,
                legend: { show: false },
                series: [{}],
                axes: [{ stroke: labelColor }, { show: false }],
            };
        }

        const uniqueUnits = new Set(series.map((s) => s.uPlotSeries.scale));
        const scalesConfig: Record<string, uPlot.Scale> = {};

        for (const unit of uniqueUnits) {
            if (!unit) continue;
            if (viewType === "fft") {
                scalesConfig[unit] = {
                    auto: true,
                    distr: 3,
                    log: 10,
                    range: (
                        u,
                        dataMin,
                        dataMax,
                    ): [number | null, number | null] => {
                        if (dataMin <= 0 || !isFinite(dataMin))
                            return [1e-4, 1];
                        const curMin = u.scales[unit]?.min ?? null;
                        const curMax = u.scales[unit]?.max ?? null;
                        let shouldResize = false;
                        if (curMin === null || curMax === null) {
                            shouldResize = true;
                        } else if (dataMin < curMin || dataMax > curMax) {
                            shouldResize = true;
                        } else {
                            const viewDec =
                                Math.log10(curMax) - Math.log10(curMin);
                            const dataDec =
                                Math.log10(dataMax) - Math.log10(dataMin);
                            if (viewDec > dataDec + 2.0) shouldResize = true;
                        }
                        if (!shouldResize) return [curMin, curMax];
                        if (dataMin === dataMax) {
                            const lv = Math.log10(dataMin);
                            return [10 ** (lv - 1), 10 ** (lv + 1)];
                        }
                        const lMin = Math.log10(dataMin);
                        const lMax = Math.log10(dataMax);
                        const lCenter = (lMin + lMax) / 2;
                        const lHalf = (lMax - lMin) / 2 + 0.5;
                        return [
                            10 ** Math.floor(lCenter - lHalf),
                            10 ** Math.ceil(lCenter + lHalf),
                        ];
                    },
                };
            } else {
                scalesConfig[unit] = { auto: true };
            }
        }

        const axesConfig: uPlot.Axis[] = [];
        if (viewType === "fft") {
            scalesConfig["x"] = { time: false, distr: 3, log: 10 };
            const formatHz = (v: number) => {
                if (v == null || !isFinite(v)) return "";
                if (v < 1e3)
                    return `${v.toFixed(v < 10 ? 2 : v < 100 ? 1 : 0)} Hz`;
                if (v < 1e6) {
                    const n = v / 1e3;
                    return `${n.toFixed(n < 10 ? 2 : n < 100 ? 1 : 0)} kHz`;
                }
                const n = v / 1e6;
                return `${n.toFixed(n < 10 ? 2 : n < 100 ? 1 : 0)} MHz`;
            };
            axesConfig.push({
                scale: "x",
                stroke: labelColor,
                values: (_u, ticks) => ticks.map(formatHz),
                grid: { stroke: gridColor },
                ticks: { stroke: gridColor },
            });
        } else {
            scalesConfig["x"] = { time: false };
            axesConfig.push({
                scale: "x",
                space: 100,
                stroke: labelColor,
                values: (_, ticks) =>
                    ticks.map((raw) =>
                        Math.abs(raw) < 1e-9
                            ? "Now"
                            : `-${Math.abs(raw).toFixed(1)}s`,
                    ),
                grid: { stroke: gridColor },
                ticks: { stroke: gridColor },
            });
        }

        let yAxisCount = 0;
        for (const unit of uniqueUnits) {
            if (!unit) continue;
            const axisOpts: uPlot.Axis = {
                scale: unit,
                label: viewType === "fft" ? `${unit}/√Hz` : unit,
                labelGap: 5,
                stroke:
                    series.find((s) => s.uPlotSeries.scale === unit)
                        ?.uPlotSeries.stroke ?? labelColor,
                values: (_, vals) =>
                    vals.map((v) => {
                        if (v == null) return "";
                        if (v === 0) return "0 ";
                        const a = Math.abs(v);
                        if (a > 0 && a < 0.01) return v.toExponential(1) + " ";
                        if (a < 10) return v.toFixed(2) + " ";
                        if (a < 100) return v.toFixed(1) + " ";
                        return v.toFixed(0) + " ";
                    }),
                grid: { stroke: gridColor },
                ticks: { stroke: gridColor },
            };
            if (yAxisCount > 0) {
                axisOpts.side = 1;
                axisOpts.grid = { show: false };
            }
            axesConfig.push(axisOpts);
            yAxisCount++;
        }

        return {
            width: 800,
            height: 400,
            series: [{}, ...series.map((s) => s.uPlotSeries)],
            scales: scalesConfig,
            axes: axesConfig,
            pxAlign: 0,
            legend: { show: false },
            cursor: {
                drag: { x: false, y: false, setScale: false },
                show: true,
                points: { show: false },
                move: (_, t, l) => [t, l],
                bind: {
                    click: () => null,
                    dblclick: () => null,
                },
            },
        };
    }

    // --- Fingerprint: only structural identity, not object references ---
    function buildFingerprint(
        series: PlotSeries[],
        viewType: string,
        dark: boolean,
    ): string {
        return (
            JSON.stringify(
                series.map((s) => [
                    s.uPlotSeries.label,
                    s.uPlotSeries.stroke,
                    s.uPlotSeries.scale,
                ]),
            ) +
            "|" +
            viewType +
            "|" +
            dark
        );
    }

    // --- Imperative uPlot instance management ---
    function createInstance(
        container: HTMLDivElement,
        series: PlotSeries[],
        viewType: "timeseries" | "fft",
        dark: boolean,
    ) {
        destroyInstance();

        const legendPlugin: uPlot.Plugin = {
            hooks: {
                setCursor: (u) => {
                    const { left, top, idx } = u.cursor;
                    if (
                        left === undefined ||
                        left < 0 ||
                        idx === undefined ||
                        idx === null ||
                        top === undefined
                    ) {
                        legendState.isActive = false;
                        return;
                    }
                    legendState.isActive = true;
                    legendState.cursorLeft = left;
                    legendState.cursorTop = top;
                    legendState.chartBounds = container.getBoundingClientRect();
                    if (viewType === "fft") {
                        legendState.frequency = u.data[0][idx];
                        legendState.values = u.data
                            .slice(1)
                            .map((d) => d[idx] ?? null);
                        legendState.relativeTime = null;
                    } else {
                        legendState.relativeTime = u.posToVal(left, "x");
                        legendState.frequency = null;
                    }
                },
                setSeries: (_u, seriesIdx) => {
                    if (seriesIdx === null) {
                        legendState.isActive = false;
                        legendState.relativeTime = null;
                    }
                },
            },
        };

        const opts = buildOptions(series, viewType, dark);
        opts.plugins = [legendPlugin];
        const inst = new uPlot(opts, [[]], container);
        uplot = inst;

        resizeObserver = new ResizeObserver((entries) => {
            if (!entries.length) return;
            const { width, height } = entries[0].contentRect;
            inst.setSize({ width, height });
        });
        resizeObserver.observe(container);

        currentFingerprint = buildFingerprint(series, viewType, dark);
    }

    function destroyInstance() {
        resizeObserver?.disconnect();
        resizeObserver = undefined;
        if (uplot) {
            uplot.destroy();
            uplot = undefined;
        }
        currentFingerprint = "";
    }

    // --- Data conversion (pure function) ---
    function toAlignedData(
        data: PlotData,
        seriesCount: number,
        fft: boolean,
    ): { aligned: (number | undefined)[][]; latestTs: number } | null {
        if (data.timestamps.length === 0 || seriesCount === 0) return null;

        let timestamps = data.timestamps;
        let seriesData = data.series_data;

        if (fft && timestamps.length > 1) {
            timestamps = timestamps.slice(1);
            seriesData = seriesData.map((s) => s.slice(1));
        }
        if (timestamps.length === 0) return null;

        const latestTs = data.timestamps[data.timestamps.length - 1];
        const xValues = fft ? timestamps : timestamps.map((t) => t - latestTs);

        const aligned: (number | undefined)[][] = [xValues];
        for (let i = 0; i < seriesCount; i++) {
            const src = seriesData[i];
            if (!src) {
                aligned.push([]);
                continue;
            }
            const col: (number | undefined)[] = new Array(src.length);
            for (let j = 0; j < src.length; j++) {
                const v = src[j];
                col[j] = v === null ? undefined : v;
            }
            aligned.push(col);
        }

        return { aligned, latestTs };
    }

    function emptyAlignedData(seriesCount: number): uPlot.AlignedData {
        return Array.from(
            { length: Math.max(seriesCount, 0) + 1 },
            (): (number | undefined)[] => [],
        ) as unknown as uPlot.AlignedData;
    }

    // --- Legend interpolation ---
    const interpolatedValues = $derived.by((): (number | null)[] => {
        const relTime = legendState.relativeTime;
        const data = rawPlotData;

        if (
            relTime === null ||
            isFFT ||
            !legendState.isActive ||
            !data ||
            data.timestamps.length === 0
        )
            return [];

        const latestAbsTimestamp = data.timestamps[data.timestamps.length - 1];
        const targetTime = latestAbsTimestamp + relTime;
        const idx = findTimestampIndex(data.timestamps, targetTime);

        if (idx === 0 || idx >= data.timestamps.length)
            return plot.series.map(() => null);

        const t1 = data.timestamps[idx - 1];
        const t2 = data.timestamps[idx];

        return data.series_data.map((series) => {
            const y1 = series[idx - 1];
            const y2 = series[idx];
            return lerp(t1, y1, t2, y2, targetTime);
        });
    });

    $effect(() => {
        if (!chartContainer) return;

        untrack(() => {
            createInstance(chartContainer, plot.series, plot.viewType, isDark);
        });

        return () => {
            destroyInstance();
        };
    });

    $effect(() => {
        const series = plot.series;
        const viewType = plot.viewType;
        const dark = isDark;

        // Skip the first run — Effect 1 handles initial creation
        if (!chartContainer || !uplot) return;

        const fp = buildFingerprint(series, viewType, dark);
        if (fp === currentFingerprint) return;

        untrack(() => {
            createInstance(chartContainer, series, viewType, dark);
        });
    });

    $effect(() => {
        const u = uplot;
        if (!u) return;

        const data = rawPlotData;
        const isPaused = plot.isPaused;
        const windowSecs = plot.windowSeconds;
        const fft = isFFT;
        const seriesCount = plot.series.length;

        if (!data || data.timestamps.length === 0 || seriesCount === 0) {
            if (plot.hasData) {
                u.setData(emptyAlignedData(seriesCount), true);
                if (!fft) {
                    u.setScale("x", {
                        min: -windowSecs,
                        max: 0,
                    });
                }
                latestTimestamp = 0;
                legendState.isActive = false;
                legendState.relativeTime = null;
                legendState.frequency = null;
                legendState.values = [];
                legendState.cursorLeft = null;
                legendState.cursorTop = null;
                legendState.chartBounds = null;
                plot.hasData = false;
            }
            return;
        }

        const result = toAlignedData(data, seriesCount, fft);
        if (!result) return;

        plot.hasData = true;

        if (!isPaused) {
            latestTimestamp = result.latestTs;
            if (!fft) {
                const oldestRelTime = result.aligned[0][0] ?? 0;
                u.setScale("x", {
                    min: Math.max(-windowSecs, oldestRelTime),
                    max: 0,
                });
            }
        }

        u.setData(result.aligned as uPlot.AlignedData, fft);
    });
</script>

<div class="relative h-full w-full">
    <div bind:this={chartContainer} class="min-h-0 h-full w-full"></div>
</div>

{#if legendState.isActive && legendState.chartBounds}
    <CustomLegend
        isActive={legendState.isActive}
        viewType={plot.viewType}
        relativeTime={legendState.relativeTime}
        frequency={legendState.frequency}
        series={plot.series}
        values={isFFT ? legendState.values : interpolatedValues}
        cursorLeft={legendState.cursorLeft}
        cursorTop={legendState.cursorTop}
        chartBounds={legendState.chartBounds}
    />
{/if}
