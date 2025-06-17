<script lang="ts">
	import uPlot from 'uplot';
	import 'uplot/dist/uPlot.min.css';

	let { options, data, chart = $bindable() }: 
		{ options: uPlot.Options; data: uPlot.AlignedData; chart?: uPlot } 
		= $props();

	let chartContainer: HTMLElement;

	$effect(() => {
		if (!chart && chartContainer && data && data[0].length > 0) {
			chart = new uPlot(options, data, chartContainer);
		} else if (chart) {
			chart.setData(data, false);
		}
	});

	$effect(() => {
		return () => {
			chart?.destroy();
		};
	});
</script>

<div bind:this={chartContainer}></div>