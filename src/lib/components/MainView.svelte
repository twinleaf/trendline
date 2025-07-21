<script lang="ts">
	import * as Resizable from '$lib/components/ui/resizable/index.js';
    import DeviceControls from '$lib/components/device-controls/DeviceControls.svelte';
    import ChartView from '$lib/components/chart-area/ChartView.svelte';
	import { cn } from '$lib/utils';

	let isCollapsed = $state(false);
</script>

<Resizable.PaneGroup direction="horizontal" class="h-full w-full">
	<Resizable.Pane
		defaultSize={25}
		minSize={10}
		maxSize={40}
		collapsible
		collapsedSize={0}
		onCollapse={() => (isCollapsed = true)}
		onExpand={() => (isCollapsed = false)}
		style={`min-width: ${isCollapsed ? 'auto' : '400px'};`}
	>
		<div
			class={cn('h-full p-4 transition-all duration-300 ease-in-out', {
				'opacity-0 -translate-x-4': isCollapsed
			})}
		>
			<DeviceControls />
		</div>
	</Resizable.Pane>
	<Resizable.Handle withHandle />
	<Resizable.Pane defaultSize={75}>
		<div class="flex h-full items-start">
			<ChartView />
		</div>
	</Resizable.Pane>
</Resizable.PaneGroup>