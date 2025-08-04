<script lang="ts">
	import 'uplot/dist/uPlot.min.css';
	import '../app.css';
	import StatusFooter from '$lib/components/StatusFooter.svelte';

	import DeviceSelectorDialog from '$lib/components/device-select/DeviceSelectorDialog.svelte';

	import { uiState } from '$lib/states/uiState.svelte';
	import { onMount } from 'svelte';
	import type { Snippet } from 'svelte';
	import { Toaster } from "$lib/components/ui/sonner/index.js";

	let { children } = $props<{ children: Snippet }>();

	onMount(() => {
        uiState.open('discovery');
	});
</script>

<Toaster richColors/>

<div class="flex h-screen flex-col gap-1 p-4 bg-[#0c1c0d]">
	<!-- <header>
		<MenubarHeader />
	</header> -->

	<main class="flex-grow overflow-auto">
		<div class="h-full rounded-lg border bg-card p-1 shadow-sm">
			{@render children()}
		</div>
	</main>

	<footer>
		<StatusFooter />
	</footer>

	<!-- The layout now just renders the dialog components. That's it. -->
	<DeviceSelectorDialog />
	<!-- <SettingsDialog /> -->
	<!-- <ExportDialog /> -->
</div>
