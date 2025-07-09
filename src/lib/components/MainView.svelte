<script lang="ts">
	import * as Resizable from '$lib/components/ui/resizable/index.js';
    import DeviceControls from '$lib/components/device-controls/DeviceControls.svelte';
    import ChartView from '$lib/components/chart-area/ChartView.svelte';
</script>

<Resizable.PaneGroup direction="horizontal" class="h-full w-full">
	<Resizable.Pane defaultSize={25} minSize={20}>
		<div class="flex h-full items-start p-4">
            <DeviceControls/>
		</div>
	</Resizable.Pane>
	<Resizable.Handle withHandle />
	<Resizable.Pane defaultSize={75}>
		<div class="flex h-full items-start p-2">
            <ChartView/>
		</div>
	</Resizable.Pane>
</Resizable.PaneGroup>
