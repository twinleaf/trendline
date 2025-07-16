<script lang="ts">
	import type { PlotConfig } from '$lib/states/chartState.svelte';
	import type { DecimationMethod } from '$lib/bindings/DecimationMethod';
	import { Label } from '$lib/components/ui/label';
	import { RadioGroup, RadioGroupItem } from '$lib/components/ui/radio-group';
	import { Separator } from "$lib/components/ui/separator/index.js";

	type Props = {
		plot: PlotConfig;
	};

	let { plot = $bindable() }: Props = $props();

	const decimationMethods: { value: DecimationMethod; label: string; description: string }[] = [
		{ value: 'None', label: 'None', description: 'No downsampling. Raw data is rendered.' },
		{
			value: 'MinMax',
			label: 'Min/Max',
			description: 'Selects min/max values per bucket. Best for multiple series.'
		},
		{
			value: 'Fpcs',
			label: 'FPCS',
			description: 'Highest visual fidelity for a single series.'
		}
	];

	const windowSecondsOptions: { value: number; label: string }[] = [
		{ value: 10, label: '10s' },
		{ value: 30, label: '30s' },
		{ value: 60, label: '1m' },
		{ value: 120, label: '2m' }
	];

	const fftWindowOptions: { value: number; label: string }[] = [
		{ value: 1, label: '1s' },
		{ value: 2, label: '2s' },
		{ value: 5, label: '5s' },
		{ value: 10, label: '10s' }
	];
</script>

<div class="max-h-[50vh] space-y-6 overflow-y-auto p-4">
	<div>
		<h4 class="font-medium leading-none">Decimation</h4>
		<p class="mt-1 text-sm text-muted-foreground">
			Select a downsampling method for performance and visual fidelity.
		</p>
		<RadioGroup bind:value={plot.decimationMethod} class="mt-2 grid gap-2">
			{#each decimationMethods as method}
				<Label
					class="flex cursor-pointer items-center gap-3 rounded-md border p-3 hover:bg-accent hover:text-accent-foreground has-[:checked]:border-primary"
				>
					<RadioGroupItem value={method.value} id={`dec-${method.value}`} />
					<div class="grid flex-1 gap-1.5 leading-normal">
						<span class="font-semibold">{method.label}</span>
						<p class="text-sm text-muted-foreground">{method.description}</p>
					</div>
				</Label>
			{/each}
		</RadioGroup>
	</div>
	<Separator />
	<div>
		<h4 class="font-medium leading-none">Time Window</h4>
		<p class="mt-1 text-sm text-muted-foreground">
			Select duration for timeseries display and FFT calculation.
		</p>
		<div class="mt-4 grid grid-cols-2 gap-x-4">
			<div class="space-y-2">
				<div class="flex items-center justify-between">
					<Label class="text-sm font-medium">Timeseries</Label>
					{#if plot.viewType === 'fft'}
						<span class="text-xs text-muted-foreground">Inactive</span>
					{/if}
				</div>
				<RadioGroup
					bind:value={() => `${plot.windowSeconds}`, (v) => (plot.windowSeconds = Number(v))}
					class="grid gap-2"
					disabled={plot.viewType === 'fft'}
				>
					{#each windowSecondsOptions as option}
						<Label
							class={`flex items-center gap-3 rounded-md border p-3 ${
								plot.viewType === 'fft'
									? 'cursor-not-allowed opacity-50'
									: 'cursor-pointer hover:bg-accent hover:text-accent-foreground has-[:checked]:border-primary'
							}`}
						>
							<RadioGroupItem value={`${option.value}`} id={`win-${option.value}`} />
							<span class="w-full font-semibold">{option.label}</span>
						</Label>
					{/each}
				</RadioGroup>
			</div>

			<div class="space-y-2">
				<div class="flex items-center justify-between">
					<Label class="text-sm font-medium">FFT</Label>
					{#if plot.viewType === 'timeseries'}
						<span class="text-xs text-muted-foreground">Inactive</span>
					{/if}
				</div>
				<RadioGroup
					bind:value={() => `${plot.fftSeconds}`, (v) => (plot.fftSeconds = Number(v))}
					class="grid gap-2"
					disabled={plot.viewType === 'timeseries'}
				>
					{#each fftWindowOptions as option}
						<Label
							class={`flex items-center gap-3 rounded-md border p-3 ${
								plot.viewType === 'timeseries'
									? 'cursor-not-allowed opacity-50'
									: 'cursor-pointer hover:bg-accent hover:text-accent-foreground has-[:checked]:border-primary'
							}`}
						>
							<RadioGroupItem value={`${option.value}`} id={`fft-win-${option.value}`} />
							<span class="w-full font-semibold">{option.label}</span>
						</Label>
					{/each}
				</RadioGroup>
			</div>
		</div>
	</div>
</div>