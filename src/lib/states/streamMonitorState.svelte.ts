import type { RowSelectionState, ExpandedState } from "@tanstack/table-core";
import { Channel, invoke } from "@tauri-apps/api/core";
import type { PipelineId } from "$lib/bindings/PipelineId";
import type { ColumnStatistics } from "$lib/bindings/ColumnStatistics";
import type { UiColumnKey } from "$lib/bindings/UiColumnKey";
import { SvelteMap } from "svelte/reactivity";
import { isVirtualKey } from "$lib/utils";

class StreamMonitorState {
  // --- Public State ---
  rowSelection = $state<RowSelectionState>({});
  expansion = $state<ExpandedState>({});
  statisticsData = new SvelteMap<string, ColumnStatistics | null>();

  // --- Private State ---
  #pipelineIds = new SvelteMap<string, PipelineId>();
  #syncQueue = Promise.resolve();

  setExpansion(nextExpansion: ExpandedState) {
    this.expansion = nextExpansion;
  }

  async setRowSelection(nextSelection: RowSelectionState) {
    this.rowSelection = nextSelection;

    const syncPromise = this.#syncQueue.then(() =>
      this.syncBackendPipelines(nextSelection),
    );

    this.#syncQueue = syncPromise.catch((error) => {
      console.error("[StatsState] Failed to sync backend pipelines:", error);
    });

    return syncPromise;
  }

  private async syncBackendPipelines(currentSelection: RowSelectionState) {
    const selectedKeys = new Set(
      Object.keys(currentSelection).filter((k) => k.startsWith("{")),
    );
    const currentKeys = new Set(this.#pipelineIds.keys());

    const keysToAdd = [...selectedKeys].filter((k) => !currentKeys.has(k));
    const keysToRemove = [...currentKeys].filter((k) => !selectedKeys.has(k));

    if (keysToAdd.length === 0 && keysToRemove.length === 0) {
      return;
    }

    for (const keyStr of keysToAdd) {
      let pipelineId: PipelineId | null = null;
      try {
        const dataKey: UiColumnKey = JSON.parse(keyStr);
        if (isVirtualKey(dataKey)) continue;
        pipelineId = await invoke<PipelineId>("create_statistics_provider", {
          sourceKey: dataKey,
          windowSeconds: 1.0,
        });

        this.statisticsData.set(keyStr, null);

        const channel = new Channel<ColumnStatistics>();
        channel.onmessage = (stats) => this.statisticsData.set(keyStr, stats);

        await invoke("listen_to_statistics", {
          id: pipelineId,
          onEvent: channel,
        });

        this.#pipelineIds.set(keyStr, pipelineId);
      } catch (e) {
        console.error(`Failed to create provider for ${keyStr}:`, e);
        this.statisticsData.delete(keyStr);
        if (pipelineId) {
          try {
            await invoke("destroy_processor", { id: pipelineId });
          } catch (destroyError) {
            console.error(
              `Failed to clean up provider for ${keyStr}:`,
              destroyError,
            );
          }
        }
      }
    }

    for (const keyStr of keysToRemove) {
      const pipelineId = this.#pipelineIds.get(keyStr);
      if (pipelineId) {
        try {
          await invoke("destroy_processor", { id: pipelineId });
        } catch (e) {
          console.error(`Failed to destroy provider for ${keyStr}:`, e);
        }
        this.#pipelineIds.delete(keyStr);
        this.statisticsData.delete(keyStr);
      }
    }
  }

  async destroy() {
    await this.setRowSelection({}).catch((error) => {
      console.error("[StatsState] Failed to clear selection:", error);
    });

    this.statisticsData.clear();
    this.expansion = {};
  }

  /**
   * Resets the persistent statistics for a single pipeline
   */
  async resetStatisticsPipeline(dataKey: UiColumnKey) {
    const key = JSON.stringify(dataKey);
    const pipelineId = this.#pipelineIds.get(key);
    if (pipelineId) {
      await invoke("reset_by_pipeline_id", { id: pipelineId });
    }
  }

  /**
   * Resets the persistent statistics for ALL currently
   * selected and active stream monitors.
   */
  async resetAllStatistics() {
    const allIds = Array.from(this.#pipelineIds.values());
    const resetPromises = allIds.map((pipelineId) =>
      invoke("reset_by_pipeline_id", { id: pipelineId }),
    );
    await Promise.all(resetPromises);
  }
}

export const streamMonitorState = new StreamMonitorState();
