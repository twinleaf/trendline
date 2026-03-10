import { deviceState } from "$lib/states/deviceState.svelte";
import { uiState } from "$lib/states/uiState.svelte";
import { listen } from "@tauri-apps/api/event";
import type { UiColumnKey } from "$lib/bindings/UiColumnKey";
import type { DecimationMethod } from "$lib/bindings/DecimationMethod";
import type { DetrendMethod } from "$lib/bindings/DetrendMethod";
import type { PlotId } from "$lib/bindings/PlotId";
import type { PortState } from "$lib/bindings/PortState";
import type { RowSelectionState } from "@tanstack/table-core";
import type { ExpandedState } from "@tanstack/table-core";
import { untrack } from "svelte";
import { Channel, invoke } from "@tauri-apps/api/core";
import type { PlotData } from "$lib/bindings/PlotData";
import { SvelteMap } from "svelte/reactivity";
import type { UiPlotState } from "$lib/bindings/UiPlotState";
import type { ViewConfig } from "$lib/bindings/ViewConfig";

export type ChartLayout = "carousel" | "vertical" | "horizontal";
export type StreamLayout = "grouped" | "vertical" | "horizontal";
const TIMESERIES_RESOLUTION_BASELINE_MULTIPLIER = 4;

export interface PlotSeries {
  dataKey: UiColumnKey;
  uPlotSeries: uPlot.Series;
}

function buildRowSelection(dataKeys: UiColumnKey[]): RowSelectionState {
  return Object.fromEntries(
    dataKeys.map((dataKey) => [JSON.stringify(dataKey), true]),
  );
}

/**
 * A utility class to assign and retrieve consistent colors for data columns
 * based on their unique key, ensuring a stable color palette across the application.
 */
class DataColumnStyler {
  #styles = new Map<string, { color: string }>();
  #colors = [
    "#3498db",
    "#e74c3c",
    "#2ecc71",
    "#f1c40f",
    "#9b59b6",
    "#1abc9c",
    "#d35400",
    "#34495e",
    "#e67e22",
    "#16a085",
    "#c0392b",
    "#8e44ad",
  ];
  #colorIndex = 0;

  #getNextColor(): string {
    const color = this.#colors[this.#colorIndex % this.#colors.length];
    this.#colorIndex++;
    return color;
  }

  /**
   * Gets a consistent style object for a given data column.
   * If a style has not been assigned, it generates a new one.
   * @param dataKey The unique identifier for the data column.
   * @returns An object containing style properties (e.g., `{ color: string }`).
   */
  getStyle(dataKey: UiColumnKey): { color: string } {
    const key = JSON.stringify(dataKey);
    if (!this.#styles.has(key)) {
      this.#styles.set(key, { color: this.#getNextColor() });
    }
    return this.#styles.get(key)!;
  }
}

/**
 * Represents the complete state and configuration for a single plot in the chart area.
 * This includes data selection, view settings, and derived uPlot options.
 */
export class PlotConfig {
  #confirmedState = $state<UiPlotState | null>(null);
  /** The user-editable title of the plot. */
  title = $state("New Plot");
  /** State for expanded rows (devices/streams) in the TanStack Table. */
  expansion = $state<ExpandedState>({});
  /** The currently active tab in the plot's settings header ('selection' or 'settings'). */
  activeTab = $state<"selection" | "settings">("selection");
  /** A flag indicating if the plot has received any data from the backend. */
  hasData = $state(false);
  /** The most recent timestamp of data received by the plot, used for tracking data flow. */
  latestTimestamp = $state(0);
  /** A local pause state for this specific plot, which can override the global pause state. */
  isPaused = $state(false);

  id = $derived.by(() => this.#confirmedState!.plot_id);

  rowSelection = $derived.by(() =>
    buildRowSelection(this.#confirmedState!.data_keys),
  );

  dataKeys = $derived.by(() => this.#confirmedState!.data_keys);

  viewType = $derived.by((): "timeseries" | "fft" =>
    this.#confirmedState!.active_view === "Fft" ? "fft" : "timeseries",
  );

  decimationMethod = $derived.by(
    (): DecimationMethod =>
      this.#confirmedState!.timeseries_config.decimation_method,
  );

  windowSeconds = $derived.by(
    (): number => this.#confirmedState!.timeseries_config.window_seconds,
  );

  resolutionMultiplier = $derived.by(
    (): number =>
      this.#confirmedState!.timeseries_config.resolution_multiplier /
      TIMESERIES_RESOLUTION_BASELINE_MULTIPLIER,
  );

  fftSeconds = $derived.by(
    (): number => this.#confirmedState!.fft_config.window_seconds,
  );

  fftDetrendMethod = $derived.by(
    (): DetrendMethod => this.#confirmedState!.fft_config.detrend_method,
  );

  /**
   * A derived array of `PlotSeries` objects, generated from the `rowSelection` state.
   * This connects the selected data columns to their uPlot display configuration.
   */
  series = $derived.by((): PlotSeries[] => {
    const newSeries: PlotSeries[] = [];
    for (const dataKey of this.dataKeys) {
      const device = deviceState.getDevice(dataKey.route);
      const stream = device?.streams.find(
        (s) => s.meta.stream_id === dataKey.stream_id,
      );
      const column = stream?.columns.find((c) => c.index === dataKey.column_id);

      if (device && stream && column) {
        const style = dataColumnStyler.getStyle(dataKey);
        newSeries.push({
          dataKey,
          uPlotSeries: {
            label: column.name,
            stroke: style.color,
            scale: column.units,
          },
        });
      }
    }
    return newSeries;
  });

  buildViewConfig(
    overrides: Partial<{
      viewType: "timeseries" | "fft";
      decimationMethod: DecimationMethod;
      windowSeconds: number;
      resolutionMultiplier: number;
      fftSeconds: number;
      fftDetrendMethod: DetrendMethod;
    }> = {},
  ): ViewConfig {
    const viewType = overrides.viewType ?? this.viewType;
    if (viewType === "timeseries") {
      return {
        Timeseries: {
          decimation_method:
            overrides.decimationMethod ?? this.decimationMethod,
          window_seconds: overrides.windowSeconds ?? this.windowSeconds,
          resolution_multiplier:
            (overrides.resolutionMultiplier ?? this.resolutionMultiplier) *
            TIMESERIES_RESOLUTION_BASELINE_MULTIPLIER,
        },
      };
    }
    return {
      Fft: {
        window_seconds: overrides.fftSeconds ?? this.fftSeconds,
        detrend_method: overrides.fftDetrendMethod ?? this.fftDetrendMethod,
      },
    };
  }

  applyConfirmedState(nextState: UiPlotState) {
    this.#confirmedState = nextState;
  }

  /**
   * Creates a new PlotConfig instance.
   * @param initialState The backend-confirmed plot state.
   * @param title The initial title for the plot.
   * @param exp The initial row expansion state.
   * @param paused The initial pause state.
   */
  constructor(
    initialState: UiPlotState,
    title: string,
    exp: ExpandedState = {},
    paused = false,
  ) {
    this.#confirmedState = initialState;
    this.title = title;
    this.expansion = exp;
    this.isPaused = paused;
  }
}

const DEFAULT_PLOT_HEIGHT = 400;

class ChartState {
  constructor() {
    void listen<[string, PortState]>(
      "port-state-changed",
      ({ payload: [portUrl, newState] }) => {
        if (deviceState.selection?.portUrl !== portUrl) return;
        if (newState === "Disconnected" || newState === "Reconnecting") {
          this.#clearAllPlotData();
        }
      },
    );
  }

  // --- Public State ---

  /** An array of PlotConfig objects, representing all charts currently displayed. */
  plots = $state<PlotConfig[]>([]);
  /** A map from a plot's unique ID to its latest stream of PlotData. */
  plotsData = new SvelteMap<PlotId, PlotData>();
  /** A global flag to pause or resume data updates for all plots. */
  isPaused = $state(false);

  // --- Layout State ---

  /** The current layout mode. 'auto' distributes plots automatically, 'manual' allows user-defined sizes. */
  layoutMode = $state<"auto" | "manual">("auto");
  /** The height of the main chart area container in pixels, measured by a ResizeObserver. */
  containerHeight = $state(0);
  /** A record of plot IDs to their user-defined heights in pixels, used only in 'manual' mode. */
  manualLayout = $state<Record<PlotId, number>>({});
  /** True when the user is actively dragging a resize handle between panes. */
  isDragResizing = $state(false);

  // --- Private State ---

  /** A set of plot IDs that have an active data listener on the backend. */
  #listeningPlots = new Set<PlotId>();
  /** A flag used to lock actions and prevent rapid-fire calls from double-clicks. */
  #isActionLocked = false;
  /** The timeout ID for the action lock, used to release the lock after a cooldown. */
  #actionLockTimeout: number | undefined;
  /** A non-reactive cache to store the most recent data for each plot, avoiding immediate reactive triggers. */
  #latestDataCache = new Map<PlotId, PlotData>();
  /** A set of plot IDs that have received new data since the last render frame, marking them as "dirty". */
  #dirtyPlots = new Set<PlotId>();
  /** A flag to ensure the render loop is only started once. */
  #isUpdateLoopRunning = false;
  /** A per-plot counter used to ignore stale async config responses. */
  #plotConfigRequestVersion = new Map<PlotId, number>();

  // --- Private Helper for Rate Limiting ---
  /**
   * A helper to prevent rapid execution of state-mutating UI actions like adding or moving plots.
   * It executes the action immediately, then enforces a short cooldown to prevent double-clicks.
   * @param actionFn The function to execute (sync or async).
   * @param cooldown The cooldown period in milliseconds.
   */
  async #withActionLock(actionFn: () => Promise<void> | void, cooldown = 200) {
    if (this.#isActionLocked) {
      return;
    }
    this.#isActionLocked = true;

    try {
      await actionFn();
    } finally {
      this.#actionLockTimeout = window.setTimeout(() => {
        this.#isActionLocked = false;
      }, cooldown);
    }
  }

  /**
   * Starts a single, global requestAnimationFrame loop to process data updates.
   * This decouples high-frequency data arrival from the UI render frequency.
   */
  #startUpdateLoop() {
    if (this.#isUpdateLoopRunning) return;
    this.#isUpdateLoopRunning = true;

    const loop = () => {
      if (this.#dirtyPlots.size > 0) {
        untrack(() => {
          for (const plotId of this.#dirtyPlots) {
            const data = this.#latestDataCache.get(plotId);
            if (data) {
              this.plotsData.set(plotId, data);
            }
          }
          this.#dirtyPlots.clear();
        });
      }
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }

  // --- Private helpers for action methods ---

  #clearPlotData(plot: PlotConfig) {
    this.plotsData.delete(plot.id);
    this.#latestDataCache.delete(plot.id);
    this.#dirtyPlots.delete(plot.id);
    plot.hasData = false;
    plot.latestTimestamp = 0;
  }

  #clearAllPlotData() {
    for (const plot of this.plots) {
      this.#clearPlotData(plot);
    }
  }

  #applyConfirmedPlotState(plot: PlotConfig, state: UiPlotState) {
    plot.applyConfirmedState(state);
  }

  #createPlotFromConfig(
    config: UiPlotState,
    title: string,
    expansion: ExpandedState = {},
    paused = false,
  ): PlotConfig {
    return new PlotConfig(config, title, expansion, paused);
  }

  #beginPlotConfigRequest(plotId: PlotId): number {
    const next = (this.#plotConfigRequestVersion.get(plotId) ?? 0) + 1;
    this.#plotConfigRequestVersion.set(plotId, next);
    return next;
  }

  async #requestPlotConfig(
    plot: PlotConfig,
    command: string,
    args: Record<string, unknown>,
  ): Promise<UiPlotState | null> {
    const requestVersion = this.#beginPlotConfigRequest(plot.id);
    try {
      const config = await invoke<UiPlotState>(command, args);
      if (this.#plotConfigRequestVersion.get(plot.id) !== requestVersion) {
        return null;
      }
      return config;
    } catch (e) {
      if (this.#plotConfigRequestVersion.get(plot.id) !== requestVersion) {
        return null;
      }
      const message =
        typeof e === "string" ? e : `Failed to update "${plot.title}".`;
      uiState.showError(message);
      return null;
    }
  }

  async ensurePlotChannel(plot: PlotConfig) {
    this.#startUpdateLoop();
    if (this.#listeningPlots.has(plot.id)) return;
    this.#listeningPlots.add(plot.id);

    const plotChannel = new Channel<PlotData>();
    plotChannel.onmessage = (data) => {
      if (!plot.isPaused) {
        this.#latestDataCache.set(plot.id, data);
        this.#dirtyPlots.add(plot.id);
      }
    };

    try {
      await invoke("listen_to_plot_data", {
        plotId: plot.id,
        onEvent: plotChannel,
      });
    } catch (e) {
      this.#listeningPlots.delete(plot.id);
      console.error(`[IPC] Failed to register channel for plot ${plot.id}`, e);
    }
  }

  /**
   * A derived property that calculates the layout of plots.
   * In 'manual' mode, it returns the user-defined layout.
   * In 'auto' mode, it fits 1-4 plots within the viewport and uses a fixed height for plots beyond four, creating a scrollbar.
   */
  layout = $derived.by(() => {
    if (this.layoutMode === "manual") {
      return this.manualLayout;
    }

    const plots = this.plots;
    const containerHeight = this.containerHeight;
    const newLayout: Record<PlotId, number> = {};

    if (plots.length === 0 || containerHeight === 0) {
      return {};
    }

    if (plots.length <= 4) {
      const heightPerPlot = containerHeight / plots.length;
      for (const plot of plots) {
        newLayout[plot.id] = heightPerPlot;
      }
    } else {
      const heightForFirstFour = containerHeight / 4;
      for (let i = 0; i < plots.length; i++) {
        newLayout[plots[i].id] =
          i < 4 ? heightForFirstFour : DEFAULT_PLOT_HEIGHT;
      }
    }
    return newLayout;
  });

  /**
   * Checks if a specific plot has any data associated with it.
   * This is used to disable UI elements proactively.
   * @param plotId The ID of the plot to check.
   * @returns `true` if the plot has data, `false` otherwise.
   */
  plotHasData(plotId: PlotId): boolean {
    const data = this.plotsData.get(plotId);
    return data ? data.timestamps.length > 0 : false;
  }

  /**
   * Cleans up all plots and backend resources. Should be called when the component is destroyed.
   */
  async destroy() {
    await this.deleteAllPlots();
  }

  // --- Action methods: explicit intents that update the backend then local state ---

  async setPlotColumns(plot: PlotConfig, dataKeys: UiColumnKey[]) {
    await this.ensurePlotChannel(plot);
    const config = await this.#requestPlotConfig(plot, "update_plot_columns", {
      plotId: plot.id,
      dataKeys,
      viewConfig: plot.buildViewConfig(),
    });
    if (config) {
      this.#clearPlotData(plot);
      this.#applyConfirmedPlotState(plot, config);
    }
  }

  async setPlotSelection(plot: PlotConfig, rowSelection: RowSelectionState) {
    const dataKeys = Object.keys(rowSelection)
      .filter((key) => key.startsWith("{") && key.endsWith("}"))
      .map((key) => JSON.parse(key) as UiColumnKey);
    await this.setPlotColumns(plot, dataKeys);
  }

  async setPlotViewType(plot: PlotConfig, viewType: "timeseries" | "fft") {
    if (plot.viewType === viewType) return;
    const viewConfig = plot.buildViewConfig({ viewType });
    const config = await this.#requestPlotConfig(
      plot,
      "update_plot_view_config",
      {
        plotId: plot.id,
        viewConfig,
      },
    );
    if (config) {
      this.#clearPlotData(plot);
      this.#applyConfirmedPlotState(plot, config);
    }
  }

  async setPlotDecimation(plot: PlotConfig, method: DecimationMethod) {
    if (plot.decimationMethod === method) return;
    const viewConfig = plot.buildViewConfig({
      decimationMethod: method,
    });
    const config = await this.#requestPlotConfig(
      plot,
      "update_plot_view_config",
      {
        plotId: plot.id,
        viewConfig,
      },
    );
    if (config) {
      this.#clearPlotData(plot);
      this.#applyConfirmedPlotState(plot, config);
    }
  }

  async setPlotWindow(plot: PlotConfig, seconds: number) {
    if (plot.windowSeconds === seconds) return;
    const viewConfig = plot.buildViewConfig({
      windowSeconds: seconds,
    });
    const config = await this.#requestPlotConfig(
      plot,
      "update_plot_view_config",
      {
        plotId: plot.id,
        viewConfig,
      },
    );
    if (config) {
      this.#clearPlotData(plot);
      this.#applyConfirmedPlotState(plot, config);
    }
  }

  async setPlotResolution(plot: PlotConfig, multiplier: number) {
    if (plot.resolutionMultiplier === multiplier) return;
    const viewConfig = plot.buildViewConfig({
      resolutionMultiplier: multiplier,
    });
    const config = await this.#requestPlotConfig(
      plot,
      "update_plot_view_config",
      {
        plotId: plot.id,
        viewConfig,
      },
    );
    if (config) {
      this.#clearPlotData(plot);
      this.#applyConfirmedPlotState(plot, config);
    }
  }

  async setPlotFftWindow(plot: PlotConfig, seconds: number) {
    if (plot.fftSeconds === seconds) return;
    const viewConfig = plot.buildViewConfig({ fftSeconds: seconds });
    const config = await this.#requestPlotConfig(
      plot,
      "update_plot_view_config",
      {
        plotId: plot.id,
        viewConfig,
      },
    );
    if (config) {
      this.#clearPlotData(plot);
      this.#applyConfirmedPlotState(plot, config);
    }
  }

  async setPlotDetrend(plot: PlotConfig, method: DetrendMethod) {
    if (plot.fftDetrendMethod === method) return;
    const viewConfig = plot.buildViewConfig({
      fftDetrendMethod: method,
    });
    const config = await this.#requestPlotConfig(
      plot,
      "update_plot_view_config",
      {
        plotId: plot.id,
        viewConfig,
      },
    );
    if (config) {
      this.#clearPlotData(plot);
      this.#applyConfirmedPlotState(plot, config);
    }
  }

  /**
   * Tears down a plot on the backend (pipeline + channel + snapshot cleanup).
   * @param plotId The ID of the plot to destroy on the backend.
   */
  async destroyPlotOnBackend(plotId: PlotId) {
    this.#listeningPlots.delete(plotId);
    this.#latestDataCache.delete(plotId);
    this.#dirtyPlots.delete(plotId);
    this.#plotConfigRequestVersion.delete(plotId);
    try {
      await invoke("destroy_plot", { plotId });
    } catch (e) {
      console.error(`[Frontend] Failed to destroy backend plot ${plotId}:`, e);
    }
  }

  // --- Public Methods ---

  /**
   * Gets the array index of a plot by its ID.
   * @param plotId The ID of the plot.
   * @returns The index of the plot, or -1 if not found.
   */
  getPlotIndex(plotId: PlotId): number {
    return this.plots.findIndex((p) => p.id === plotId);
  }

  /**
   * Adds a new, empty plot to the end of the chart list.
   */
  async addPlot() {
    await this.#withActionLock(async () => {
      const config = await invoke<UiPlotState>("create_plot");
      const initialExpansion: ExpandedState = {};
      for (const portData of deviceState.devices) {
        for (const device of portData.devices)
          initialExpansion[`${device.url}:${device.route}`] = true;
      }
      const newPlot = this.#createPlotFromConfig(
        config,
        "New Plot",
        initialExpansion,
        this.isPaused,
      );
      this.plots.push(newPlot);
      if (this.layoutMode === "manual") {
        const newLayout = { ...this.manualLayout };
        newLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
        this.manualLayout = newLayout;
      }
    });
  }

  /**
   * Adds a new, empty plot immediately above a specified plot.
   * @param plotId The ID of the plot to add the new plot above.
   */
  async addPlotAbove(plotId: PlotId) {
    await this.#withActionLock(async () => {
      const index = this.getPlotIndex(plotId);
      if (index === -1) return;
      const config = await invoke<UiPlotState>("create_plot");
      const initialExpansion: ExpandedState = {};
      for (const portData of deviceState.devices) {
        for (const device of portData.devices)
          initialExpansion[`${device.url}:${device.route}`] = true;
      }
      const newPlot = this.#createPlotFromConfig(
        config,
        "New Plot",
        initialExpansion,
        this.isPaused,
      );
      this.plots.splice(index, 0, newPlot);
      if (this.layoutMode === "manual") {
        const newLayout = { ...this.manualLayout };
        newLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
        this.manualLayout = newLayout;
      }
    });
  }

  /**
   * Adds a new, empty plot immediately below a specified plot.
   * @param plotId The ID of the plot to add the new plot below.
   */
  async addPlotBelow(plotId: PlotId) {
    await this.#withActionLock(async () => {
      const index = this.getPlotIndex(plotId);
      if (index === -1) return;
      const config = await invoke<UiPlotState>("create_plot");
      const initialExpansion: ExpandedState = {};
      for (const portData of deviceState.devices) {
        for (const device of portData.devices)
          initialExpansion[`${device.url}:${device.route}`] = true;
      }
      const newPlot = this.#createPlotFromConfig(
        config,
        "New Plot",
        initialExpansion,
        this.isPaused,
      );
      this.plots.splice(index + 1, 0, newPlot);
      if (this.layoutMode === "manual") {
        const newLayout = { ...this.manualLayout };
        newLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
        this.manualLayout = newLayout;
      }
    });
  }

  /**
   * Adds a new plot that is pre-configured with a selected data column.
   * @param dataKey The data column identifier to pre-select.
   * @param streamName The name of the stream for the plot title.
   */
  async addPlotFromStream(dataKey: UiColumnKey, streamName: string) {
    await this.#withActionLock(async () => {
      const config = await invoke<UiPlotState>("create_plot");
      const selectionKey = JSON.stringify(dataKey);
      const portUrl = deviceState.selection?.portUrl ?? "";
      const deviceId = `${portUrl}:${dataKey.route}`;
      const streamId = `${deviceId}:${dataKey.stream_id}`;
      const initialExpansion: ExpandedState = {
        [deviceId]: true,
        [streamId]: true,
      };
      const newPlot = this.#createPlotFromConfig(
        config,
        `${streamName}`,
        initialExpansion,
        this.isPaused,
      );
      this.plots.push(newPlot);
      await this.setPlotSelection(newPlot, { [selectionKey]: true });
      if (this.layoutMode === "manual") {
        const newLayout = { ...this.manualLayout };
        newLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
        this.manualLayout = newLayout;
      }
    });
  }

  /**
   * Moves a plot up or down in the list.
   * @param plotId The ID of the plot to move.
   * @param direction The direction to move the plot, either 'up' or 'down'.
   */
  movePlot(plotId: PlotId, direction: "up" | "down") {
    this.#withActionLock(() => {
      const index = this.getPlotIndex(plotId);
      if (index === -1) return;
      const newIndex = direction === "up" ? index - 1 : index + 1;
      if (newIndex < 0 || newIndex >= this.plots.length) return;
      const newPlots = [...this.plots];
      const [movedPlot] = newPlots.splice(index, 1);
      newPlots.splice(newIndex, 0, movedPlot);
      this.plots = newPlots;
    });
  }

  /**
   * Copies the decimated data currently visible in the plot to the clipboard.
   * @param plotId The ID of the plot.
   */
  async copyPlotViewToClipboard(plotId: PlotId) {
    const plot = this.plots.find((p) => p.id === plotId);
    if (!plot) {
      uiState.showError("Plot not found.");
      return;
    }

    const decimatedData = this.plotsData.get(plotId);
    if (!decimatedData || decimatedData.timestamps.length === 0) {
      uiState.showError("No plot data available to copy.");
      return;
    }

    try {
      await invoke("export_plot_data_to_clipboard", {
        plotData: decimatedData,
        dataColumnIds: plot.series.map((s) => s.dataKey),
      });
    } catch (e) {
      uiState.showError(e as string);
    }
  }

  /**
   * Saves the decimated data currently visible in the plot to a CSV file.
   * @param plotId The ID of the plot whose data will be saved.
   */
  async savePlotDataAsCsv(plotId: PlotId) {
    const plot = this.plots.find((p) => p.id === plotId);
    if (!plot) {
      uiState.showError("Plot not found.");
      return;
    }

    const decimatedData = this.plotsData.get(plotId);
    if (!decimatedData || decimatedData.timestamps.length === 0) {
      uiState.showError("No plot data available to save.");
      return;
    }

    try {
      await invoke("save_plot_data_to_file", {
        plotData: decimatedData,
        dataColumnIds: plot.series.map((s) => s.dataKey),
        fileNameSuggestion: `${plot.title.replace(/\s+/g, "_")}_plotted.csv`,
      });
    } catch (e) {
      uiState.showError(e as string);
    }
  }

  /**
   * Saves the high-resolution raw data corresponding to the plot's current view.
   * It intelligently uses a pre-existing snapshot if the plot is paused,
   * or performs an instant "live snapshot" if the plot is running.
   * @param plotId The ID of the plot.
   */
  async saveRawDataAsCsv(plotId: PlotId) {
    const plot = this.plots.find((p) => p.id === plotId);
    if (!plot) {
      uiState.showError("Plot not found.");
      return;
    }

    const viewData = this.plotsData.get(plotId);
    if (!viewData || viewData.timestamps.length === 0) {
      uiState.showError("No data in view to define a time range for saving.");
      return;
    }

    const startTime = Math.min(...viewData.timestamps);
    const endTime = Math.max(...viewData.timestamps);

    try {
      uiState.showInfo("Fetching and preparing raw data...");
      await invoke("save_raw_plot_data_to_file", {
        plotId: plot.id,
        dataColumnIds: plot.series.map((s) => s.dataKey),
        startTime,
        endTime,
        isPaused: plot.isPaused,
        fileNameSuggestion: `${plot.title.replace(/\s+/g, "_")}_raw.csv`,
      });
    } catch (e) {
      uiState.showError(e as string);
    }
  }

  /**
   * Resets a plot: clears all pipeline state and plot data, keeping columns selected.
   * Data refills live from the current moment.
   */
  async resetPlot(plotId: PlotId) {
    const plot = this.plots.find((p) => p.id === plotId);
    if (!plot) return;
    try {
      await invoke("reset_plot", { plotId });
      this.#clearPlotData(plot);
    } catch (e) {
      uiState.showError(e as string);
    }
  }

  /**
   * Removes a plot from the chart area and intelligently updates the layout.
   * Awaits backend teardown before removing from local state.
   * @param plotId The ID of the plot to remove.
   */
  async removePlot(plotId: PlotId) {
    const plotIndex = this.plots.findIndex((p) => p.id === plotId);
    if (plotIndex === -1) return;

    const oldManualLayout = { ...this.manualLayout };
    const wasInManualMode = this.layoutMode === "manual";

    await this.destroyPlotOnBackend(plotId);

    this.plots.splice(plotIndex, 1);
    this.plotsData.delete(plotId);

    if (wasInManualMode) {
      if (this.plots.length <= 1) {
        this.rebalancePlots();
        return;
      }
      const newLayout: Record<PlotId, number> = {};
      const remainingSum = this.plots.reduce(
        (sum, p) => sum + (oldManualLayout[p.id] || 0),
        0,
      );
      if (remainingSum > 0 && this.containerHeight > 0) {
        const scaleFactor = this.containerHeight / remainingSum;
        for (const p of this.plots) {
          const oldHeight = oldManualLayout[p.id] || 0;
          newLayout[p.id] = oldHeight * scaleFactor;
        }
        this.manualLayout = newLayout;
      } else {
        this.rebalancePlots();
      }
    }
  }

  /**
   * Removes all plots from the chart area and resets the layout state.
   */
  async deleteAllPlots() {
    await Promise.allSettled(
      untrack(() => this.plots.map((p) => this.destroyPlotOnBackend(p.id))),
    );
    this.plots = [];
    this.manualLayout = {};
    this.plotsData.clear();
    this.layoutMode = "auto";
  }

  /**
   * Resets the plot layout to 'auto' mode, causing all plots to be resized to fit the viewport.
   */
  rebalancePlots() {
    this.layoutMode = "auto";
  }

  /**
   * Sets a plot's height to a specific percentage of the viewport height.
   * This is a direct, imperative command that does not affect the size of other plots.
   * @param plotId The ID of the plot to resize.
   * @param viewPercentage The desired height as a percentage of the viewport (e.g., 50).
   */
  setPlotHeight(plotId: PlotId, viewPercentage: number) {
    this.#withActionLock(() => {
      this.switchToManualMode();
      if (this.containerHeight <= 0) return;
      const newLayout = { ...this.manualLayout };
      const targetHeight = this.containerHeight * (viewPercentage / 100);
      newLayout[plotId] = targetHeight;
      this.manualLayout = newLayout;
    });
  }

  /**
   * Compares two numbers with a small tolerance.
   * @private
   */
  _isClose(a: number, b: number, epsilon = 0.1): boolean {
    return Math.abs(a - b) < epsilon;
  }

  /**
   * Gets the preset percentage key for a plot's height, used for UI checkmarks.
   * @param plotId The ID of the plot to check.
   * @returns A string key ('25', '33', '50', '100') or undefined if its height doesn't match a preset.
   */
  getPlotHeightPercentageKey(plotId: PlotId): string | undefined {
    const plotHeight = this.layout[plotId];
    if (!plotHeight || this.containerHeight <= 0) {
      return undefined;
    }
    const percentage = (plotHeight / this.containerHeight) * 100;
    if (this._isClose(percentage, 100)) return "100";
    if (this._isClose(percentage, 50)) return "50";
    if (this._isClose(percentage, 33.33)) return "33";
    if (this._isClose(percentage, 25)) return "25";
    return undefined;
  }

  /**
   * Gets the current type of plot. Used for validating context menu options.
   * @param plotId The ID of the plot to check.
   * @returns A string key ('timeseries', 'fft') or undefined if the plot is not found.
   */
  getPlotType(plotId: PlotId): "timeseries" | "fft" | undefined {
    const plot = this.plots.find((p) => p.id === plotId);
    return plot?.viewType;
  }

  /**
   * Switches the layout to 'manual' mode, taking a snapshot of the current layout.
   */
  switchToManualMode() {
    if (this.layoutMode === "manual") return;
    this.manualLayout = untrack(() => this.layout);
    this.layoutMode = "manual";
  }

  /**
   * Updates the manual layout based on percentage values from the resizable pane component.
   * @param percentages An array of numbers (0-100) representing the new size of each pane.
   */
  updateLayoutFromManualResize(percentages: number[]) {
    if (this.#isActionLocked || !this.isDragResizing) return;
    const currentPlots = untrack(() => this.plots);
    if (
      this.layoutMode !== "manual" ||
      percentages.length !== currentPlots.length
    )
      return;
    const totalPixelHeight = Object.values(untrack(() => this.layout)).reduce(
      (sum, h) => sum + h,
      0,
    );
    if (totalPixelHeight === 0) return;
    const newLayout: Record<PlotId, number> = {};
    currentPlots.forEach((plot, i) => {
      newLayout[plot.id] = (totalPixelHeight * percentages[i]) / 100;
    });
    this.manualLayout = newLayout;
  }

  /**
   * Executes the backend pause/unpause logic and returns whether it succeeded.
   * @param plot The plot to update.
   * @param pause The desired new state (true for pause, false for unpause).
   * @returns `true` if the backend call succeeded, `false` otherwise.
   */
  private async _setPlotPauseOnBackend(
    plot: PlotConfig,
    pause: boolean,
  ): Promise<boolean> {
    try {
      if (pause) {
        const viewData = this.plotsData.get(plot.id);
        if (!viewData || viewData.timestamps.length === 0) {
          uiState.showWarning(
            `Cannot pause "${plot.title}": plot has no data.`,
          );
          return false;
        }
        const startTime = Math.min(...viewData.timestamps);
        const endTime = Math.max(...viewData.timestamps);
        await invoke("pause_plot", { plotId: plot.id, startTime, endTime });
      } else {
        await invoke("unpause_plot", { plotId: plot.id });
      }
      return true;
    } catch (e) {
      uiState.showError(e as string);
      return false;
    }
  }

  /**
   * Sets the pause state for a SINGLE plot. Awaits backend before updating UI.
   */
  async setLocalPlotPause(plot: PlotConfig, paused: boolean) {
    if (plot.isPaused === paused) return;
    if (await this._setPlotPauseOnBackend(plot, paused)) {
      plot.isPaused = paused;
    }
  }

  /**
   * Toggles the pause state for a SINGLE plot. This is called by the UI.
   */
  async handleLocalPlotToggle(plot: PlotConfig) {
    await this.setLocalPlotPause(plot, !plot.isPaused);
  }

  /**
   * Toggles the GLOBAL pause state, acting as a "Pause/Play All" button.
   * Awaits all backend calls and only updates state for successful operations.
   */
  async toggleGlobalPause() {
    const newState = !this.isPaused;
    const toUpdate = this.plots.filter((p) => p.isPaused !== newState);
    const results = await Promise.allSettled(
      toUpdate.map((plot) => this._setPlotPauseOnBackend(plot, newState)),
    );
    let allOk = true;
    for (let i = 0; i < toUpdate.length; i++) {
      const r = results[i];
      if (r.status === "fulfilled" && r.value) {
        toUpdate[i].isPaused = newState;
      } else {
        allOk = false;
      }
    }
    if (allOk) this.isPaused = newState;
  }
}

/** A singleton instance of the DataColumnStyler for global use. */
export const dataColumnStyler = new DataColumnStyler();
/** A global class state for manipulating ChartView plots and their configuration. */
export const chartState = new ChartState();
