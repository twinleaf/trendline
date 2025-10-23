<script lang="ts">
	import { SvelteFlowProvider } from '@xyflow/svelte';
	import Flow from './Flow.svelte';
	import DnDProvider from './DnDProvider.svelte';
	
</script>

<div class="flex h-full w-full flex-col rounded-lg border bg-card p-4 text-card-foreground">
	<SvelteFlowProvider>
		<DnDProvider>
			<Flow />
		</DnDProvider>
	</SvelteFlowProvider>
</div>