<script lang="ts">
	import * as Resizable from '$lib/components/ui/resizable/index.js';
	import DeviceControls from '$lib/components/device-controls/DeviceControls.svelte';
	import StreamMonitor from '$lib/components/stream-monitor/StreamMonitor.svelte';
	import MathChannels from '$lib/components/math-channels/MathChannels.svelte';
	import ChartView from '$lib/components/chart-area/ChartView.svelte';
	import { cn } from '$lib/utils';
	import * as Select from '$lib/components/ui/select/index.js';
	import { tick } from 'svelte';
	import { chartState } from '$lib/states/chartState.svelte';

	let isCollapsed = $state(false);
	type PanelId = 'device-controls' | 'stream-monitor' | 'math-channels';

	interface Panel {
		value: PanelId;
		label: string;
		hotkey: string;
	}

	const panels: Panel[] = [
		{ value: 'device-controls', label: 'Device Controls', hotkey: '1' },
		{ value: 'stream-monitor', label: 'Stream Monitor', hotkey: '2' },
		{ value: 'math-channels', label: 'Math Channels', hotkey: '3' }
	];

	let panelElements = $state<Record<string, HTMLDivElement>>({});
	let selectedPanel = $state(panels[0].value);

	const triggerContent = $derived(
		panels.find((p) => p.value === selectedPanel)?.label ?? 'Select a panel'
	);

	async function handleKeydown(event: KeyboardEvent) {
        if (
            event.ctrlKey ||
            event.altKey ||
            event.shiftKey ||
            event.metaKey ||
            event.target instanceof HTMLInputElement ||
            event.target instanceof HTMLTextAreaElement
        ) {
            return;
        }

        switch (event.key) {
            case ' ':
                event.preventDefault();
                chartState.toggleGlobalPause(); 
                break;

            case '1':
            case '2':
            case '3':
                const panel = panels.find((p) => p.hotkey === event.key);
                if (panel) {
                    event.preventDefault();
                    selectedPanel = panel.value;
                    await tick();
                    panelElements[panel.value]?.focus();
                }
                break;
        }
    }
</script>

<svelte:window on:keydown={handleKeydown} />

{#snippet deviceControlsSnippet()}
	<DeviceControls />
{/snippet}

{#snippet streamMonitorSnippet()}
	<StreamMonitor />
{/snippet}

{#snippet mathChannelsSnippet()}
	<MathChannels />
{/snippet}

<Resizable.PaneGroup direction="horizontal" class="h-full w-full">
	{@const panelContent = {
		'device-controls': deviceControlsSnippet,
		'stream-monitor': streamMonitorSnippet,
		'math-channels': mathChannelsSnippet
	}}
	<Resizable.Pane
		defaultSize={25}
		minSize={10}
		maxSize={40}
		collapsible
		collapsedSize={0}
		onCollapse={() => (isCollapsed = true)}
		onExpand={() => (isCollapsed = false)}
		style={`min-width: ${isCollapsed ? 'auto' : '500px'}`}
	>
		<div
			class={cn('h-full p-4 transition-all duration-300 ease-in-out', {
				'opacity-0 -translate-x-4 pointer-events-none': isCollapsed
			})}
		>
			<div class="flex h-full flex-col gap-4">
				<Select.Root type="single" name="panelSelector" bind:value={selectedPanel}>
					<Select.Trigger class="w-full text-lg font-semibold h-12">
						<div class="flex items-center justify-between w-full">
							<span>{triggerContent}</span>
							<div class="flex items-center gap-2">
								<kbd
									class="pointer-events-none inline-flex h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium text-muted-foreground opacity-100"
								>
									{panels.find((p) => p.value === selectedPanel)?.hotkey}
								</kbd>
							</div>
						</div>
					</Select.Trigger>
					<Select.Content>
						<Select.Group>
							<Select.Label>Panels</Select.Label>
							{#each panels as panel (panel.value)}
								<Select.Item value={panel.value} label={panel.label}>
									<div class="flex items-center justify-between w-full">
										<span>{panel.label}</span>
										<kbd
											class="ml-auto inline-flex h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium text-muted-foreground"
										>
											{panel.hotkey}
										</kbd>
									</div>
								</Select.Item>
							{/each}
						</Select.Group>
					</Select.Content>
				</Select.Root>

				<div class="flex-1 min-h-0 relative">
					{#each panels as panel (panel.value)}
						<div
							bind:this={panelElements[panel.value]}
							tabindex="-1"
							class="absolute inset-0 rounded-lg focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 focus:ring-offset-background"
							style:display={selectedPanel === panel.value ? 'block' : 'none'}
						>
							{@render panelContent[panel.value]()}
						</div>
					{/each}
				</div>
			</div>
		</div>
	</Resizable.Pane>
	<Resizable.Handle withHandle />
	<Resizable.Pane defaultSize={75}>
		<div class="flex h-full items-start">
			<ChartView />
		</div>
	</Resizable.Pane>
</Resizable.PaneGroup>