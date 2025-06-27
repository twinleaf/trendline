<script lang="ts">
	import 'uplot/dist/uPlot.min.css';
	import '../app.css';
	import MenubarHeader from '$lib/components/MenubarHeader.svelte';
	import StatusFooter from '$lib/components/StatusFooter.svelte';

	import DeviceSelectorDialog from '$lib/components/device-select/DeviceSelectorDialog.svelte';

	import { uiStore } from '$lib/stores/ui.store.svelte';
	import { onMount } from 'svelte';
	import type { Snippet } from 'svelte';

	let { children } = $props<{ children: Snippet }>();

	onMount(() => {
        uiStore.open('discovery');
	});
</script>

<div class="flex h-screen flex-col gap-1 p-4">
	<!-- <header>
		<MenubarHeader />
	</header> -->

	<main class="flex-grow overflow-auto">
		<div class="h-full rounded-lg border bg-card p-1 shadow-sm">
			{@render children()}
		</div>
	</main>

	<!-- <footer>
		<StatusFooter />
	</footer> -->

	<!-- The layout now just renders the dialog components. That's it. -->
	<DeviceSelectorDialog />
	<!-- <SettingsDialog /> -->
	<!-- <ExportDialog /> -->
</div>
