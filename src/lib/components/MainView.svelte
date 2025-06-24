<script lang="ts">
	import * as Resizable from '$lib/components/ui/resizable/index.js';
    import DeviceControls from '$lib/components/DeviceControls.svelte';
    import LiveChart from '$lib/components/LiveChart.svelte';
</script>

<Resizable.PaneGroup direction="horizontal" class="h-full w-full">
	<Resizable.Pane defaultSize={25} minSize={20}>
		<div class="flex h-full items-start p-4">
            <DeviceControls/>
		</div>
	</Resizable.Pane>
	<Resizable.Handle withHandle />
	<Resizable.Pane defaultSize={75}>
		<div class="flex h-full items-center justify-center p-2">
            <LiveChart/>
		</div>
	</Resizable.Pane>
</Resizable.PaneGroup>
