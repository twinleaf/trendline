import type { RowSelectionState, ExpandedState } from '@tanstack/table-core';
import { Channel, invoke } from '@tauri-apps/api/core';
import type { PipelineId } from '$lib/bindings/PipelineId';
import type { StreamStatistics } from '$lib/bindings/StreamStatistics';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import { SvelteMap } from 'svelte/reactivity';
import { untrack } from 'svelte';

class StreamMonitorState {
	// --- Public State ---
	rowSelection = $state<RowSelectionState>({});
	expansion = $state<ExpandedState>({});
	statisticsData = new SvelteMap<string, StreamStatistics | null>();

	// --- Private State ---
	#pipelineIds = new SvelteMap<string, PipelineId>();
	#listeningProviders = new Set<string>();
	#isInitialized = false;

	/**
	 * Initializes the state manager's reactive loop.
	 * This should be called once when the feature is mounted.
	 */
	init() {
		if (this.#isInitialized) return;
		this.#isInitialized = true;

		console.log('[StatsState] Initializing reactive effects.');

		$effect(() => {
			const currentSelection = this.rowSelection;
			untrack(() => {
				this.syncBackendPipelines(currentSelection);
			});
		});
	}

	/**
	 * Private method to sync backend state based on the current selection.
	 * This is triggered automatically by the internal $effect.
	 */
	private async syncBackendPipelines(currentSelection: RowSelectionState) {
		const selectedKeys = new Set(Object.keys(currentSelection).filter((k) => k.startsWith('{')));
		const currentKeys = new Set(this.#pipelineIds.keys());

		const keysToAdd = [...selectedKeys].filter((k) => !currentKeys.has(k));
		const keysToRemove = [...currentKeys].filter((k) => !selectedKeys.has(k));

		if (keysToAdd.length === 0 && keysToRemove.length === 0) {
			return;
		}
		
		console.log(`[StatsState] Syncing... Add: ${keysToAdd.length}, Remove: ${keysToRemove.length}`);

		for (const keyStr of keysToAdd) {
			try {
				const dataKey: DataColumnId = JSON.parse(keyStr);
				const pipelineId = await invoke<PipelineId>('create_statistics_provider', { sourceKey: dataKey, windowSeconds: 2.0 });

				this.#pipelineIds.set(keyStr, pipelineId);
				this.statisticsData.set(keyStr, null);

				if (!this.#listeningProviders.has(keyStr)) {
					const channel = new Channel<StreamStatistics>();
					channel.onmessage = (stats) => this.statisticsData.set(keyStr, stats);
					await invoke('listen_to_statistics', { id: pipelineId, onEvent: channel });
					this.#listeningProviders.add(keyStr);
				}
			} catch (e) {
				console.error(`Failed to create provider for ${keyStr}:`, e);
			}
		}

		for (const keyStr of keysToRemove) {
			const pipelineId = this.#pipelineIds.get(keyStr);
			if (pipelineId) {
				invoke('destroy_processor', { id: pipelineId });
				this.#pipelineIds.delete(keyStr);
				this.statisticsData.delete(keyStr);
				this.#listeningProviders.delete(keyStr);
			}
		}
	}

	/**
	 * Cleans up everything. Called on component unmount.
	 */
	destroy() {
		if (!this.#isInitialized) return;
		for (const pipelineId of this.#pipelineIds.values()) {
			invoke('destroy_processor', { id: pipelineId });
		}
		this.#pipelineIds.clear();
		this.statisticsData.clear();
		this.#listeningProviders.clear();
		this.rowSelection = {}; // Reset for clean re-initialization
		this.#isInitialized = false;
		console.log('[StatsState] Destroyed.');
	}

	/**
	 * Resets the persistent statistics for a single stream.
	 */
	async resetStatistics(dataKey: DataColumnId) {
		const key = JSON.stringify(dataKey);
		const pipelineId = this.#pipelineIds.get(key);
		console.log(`[StatsState] Clearing stats provider ${pipelineId}`)
		if (pipelineId) {
			await invoke('reset_statistics_provider', { id: pipelineId });
		}
	}

    /**
     * Resets the persistent statistics for ALL currently
     * selected and active stream monitors.
     */
    async resetAllStatistics() {
        console.log(`[StatsState] Resetting all ${this.#pipelineIds.size} active providers.`);
        
        // Get all the current pipeline IDs
        const allIds = Array.from(this.#pipelineIds.values());

        // Create an array of invoke promises
        const resetPromises = allIds.map(id => 
            invoke('reset_statistics_provider', { id })
        );

        // Run all reset commands in parallel and wait for them to complete
        await Promise.all(resetPromises);
        
        console.log('[StatsState] All providers reset.');
    }
}


export const streamMonitorState = new StreamMonitorState();