<script lang="ts">
    import type { PlotSeries } from "$lib/states/chartState.svelte";

    let {
        viewType,
        relativeTime,
        frequency,
        series,
        values,
        isActive,
        cursorLeft,
        cursorTop,
        chartBounds,
    } = $props<{
        viewType: "timeseries" | "fft";
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
    let style = $state("");

    const headerText = $derived.by(() => {
        if (viewType === "fft") {
            return frequency === null ? "---" : `${frequency.toFixed(2)} Hz`;
        }
        return relativeTime === null ? "---" : `${relativeTime.toFixed(3)}s`;
    });

    function formatValue(v: number | null, mode: "fft" | "timeseries") {
        if (v == null || !isFinite(v)) return "---";
        return mode === "fft"
            ? v.toExponential(2) // "1.23e+04"
            : v.toExponential(3); // "1.234e+00"
    }

    $effect(() => {
        if (isActive && cursorLeft != null && cursorTop != null && legendEl) {
            const legendRect = legendEl.getBoundingClientRect();
            if (legendRect.width === 0) return;

            const margin = 5;
            const cursorOffset = 150;
            const anchor = {
                left: chartBounds.left + cursorLeft,
                top: chartBounds.top + cursorTop,
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
            } else if (spaceOnRight >= requiredWidth) {
                left = anchor.left + cursorOffset;
            } else {
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
    <div
        bind:this={legendEl}
        class="custom-legend"
        style="position: fixed; {style}"
    >
        <div class="timestamp">{headerText}</div>
        <div class="series-values">
            {#each series as s, i (s.dataKey)}
                {#if values[i] !== undefined && values[i] !== null}
                    <div class="series-item">
                        <span
                            class="color-dot"
                            style:background-color={s.uPlotSeries.stroke}
                        ></span>
                        <span class="label">{s.uPlotSeries.label}:</span>
                        <span class="value"
                            >{formatValue(values[i], viewType)}</span
                        >
                    </div>
                {/if}
            {/each}
        </div>
    </div>
{/if}

<style>
    /* ---------- legend shell ---------- */
    .custom-legend {
        position: fixed;
        z-index: 100;
        transform: translate3d(0, 0, 0);

        background: color-mix(in srgb, var(--card) 92%, transparent);
        border: 1px solid var(--border);
        color: var(--card-foreground);
        border-radius: var(--radius);
        box-shadow: 0 1px 2px rgb(0 0 0 / 0.16);

        padding: 8px 10px;
        font-family: var(--font-sans);
        font-size: 12px;
        line-height: 1.3;
        white-space: nowrap;
        pointer-events: none;
    }

    /* ---------- header (time / freq) ---------- */
    .timestamp {
        font-family: var(--font-mono);
        font-variant-numeric: tabular-nums;
        font-weight: 600;
        color: var(--foreground);

        text-align: right;
        min-width: 10ch;
        margin-bottom: 4px;
    }

    /* ---------- series list ---------- */
    .series-values {
        display: grid;
        gap: 4px;
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
        flex: 0 0 10px;
    }

    .label {
        font-weight: 400;
        color: var(--muted-foreground);
    }

    .value {
        font-family: var(--font-mono);
        font-variant-numeric: tabular-nums;
        font-weight: 500;
        color: var(--foreground);

        text-align: right;
        min-width: 10ch; /* matches .timestamp */
    }
</style>
