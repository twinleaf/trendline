import type { RowSelectionState, ExpandedState } from '@tanstack/table-core';
import { invoke } from '@tauri-apps/api/core';
import type { PipelineId } from '$lib/bindings/PipelineId';
import type { StreamStatistics } from '$lib/bindings/StreamStatistics';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import { chartState } from './chartState.svelte';

class StreamMonitorState {
	// --- Public State ---
	rowSelection = $state<RowSelectionState>({});
	expansion = $state<ExpandedState>({});
	statisticsData = $state(new Map<string, StreamStatistics>());

	// --- Private State ---
	#pipelineIds = new Map<string, PipelineId>();
	#pollingIntervalId: ReturnType<typeof setInterval> | null = null;

	// --- "Command" Methods ---

	/**
	 * Starts the central polling loop for statistics.
	 * Should be called from the component's onMount.
	 */
	initPolling() {
		if (this.#pollingIntervalId) return; // Already running

		const POLLING_INTERVAL_MS = 100;
		let isFetching = false;

		this.#pollingIntervalId = setInterval(async () => {
			if (chartState.isPaused || this.#pipelineIds.size === 0 || isFetching) {
				return;
			}

			isFetching = true;
			try {
				const promises = Array.from(this.#pipelineIds.entries()).map(async ([key, id]) => {
					try {
						const stats = await invoke<StreamStatistics>('get_statistics_data', { id });
						this.statisticsData.set(key, stats);
					} catch (e) {
					}
				});

				await Promise.all(promises);
			} finally {
				isFetching = false;
			}
		}, POLLING_INTERVAL_MS);
	}


	/**
	 * Creates and destroys backend pipelines based on the current selection.
	 * Should be called from a component's $effect when rowSelection changes.
	 */
	updatePipelineSelection() {
		const selectedKeys = new Set(Object.keys(this.rowSelection).filter((k) => k.startsWith('{')));
		const currentKeys = new Set(this.#pipelineIds.keys());

		const keysToAdd = [...selectedKeys].filter((k) => !currentKeys.has(k));
		const keysToRemove = [...currentKeys].filter((k) => !selectedKeys.has(k));

		// Create new pipelines
		for (const keyStr of keysToAdd) {
			try {
				const dataKey = JSON.parse(keyStr);
				invoke<PipelineId>('create_statistics_provider', { sourceKey: dataKey, windowSeconds: 2.0 })
					.then((pipelineId) => {
						this.#pipelineIds.set(keyStr, pipelineId);
					})
					.catch((e) => console.error(`Failed to create stats provider for ${keyStr}:`, e));
			} catch (e) {
				console.error(`Error parsing DataColumnId key: ${keyStr}`, e);
			}
		}

		// Destroy old pipelines
		for (const keyStr of keysToRemove) {
			const pipelineId = this.#pipelineIds.get(keyStr);
			if (pipelineId) {
				invoke('destroy_processor', { id: pipelineId });
				this.#pipelineIds.delete(keyStr);
				this.statisticsData.delete(keyStr);
			}
		}
	}

	/**
	 * Stops the polling loop and destroys all backend processors.
	 * Should be called from the component's onDestroy.
	 */
	destroy() {
		if (this.#pollingIntervalId) {
			clearInterval(this.#pollingIntervalId);
			this.#pollingIntervalId = null;
		}
		for (const pipelineId of this.#pipelineIds.values()) {
			invoke('destroy_processor', { id: pipelineId });
		}
		this.#pipelineIds.clear();
		this.statisticsData.clear();
	}

	/**
	 * Resets the persistent statistics for a single stream.
	 */
	async resetStatistics(dataKey: DataColumnId) {
		const key = JSON.stringify(dataKey);
		const pipelineId = this.#pipelineIds.get(key);
		if (pipelineId) {
			await invoke('reset_statistics_provider', { id: pipelineId });
		}
	}
}

export const streamMonitorState = new StreamMonitorState();