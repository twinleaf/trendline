<script lang="ts">
	import type { PlotConfig } from '$lib/states/chartState.svelte';
	import { columns, type TreeRow } from '$lib/components/chart-area/data-table/column';
	import DataTable from '$lib/components/chart-area/data-table/DataTable.svelte';
	import PlotSettings from '$lib/components/chart-area/PlotSettings.svelte';
	import { Button } from '$lib/components/ui/button/';
	import * as Popover from '$lib/components/ui/popover';
	import { ScrollArea } from '$lib/components/ui/scroll-area/index.js';
	import * as Tabs from '$lib/components/ui/tabs';
	import { Settings } from '@lucide/svelte';

	type Props = {
		plot: PlotConfig;
		treeData: TreeRow[];
	};

	let { plot = $bindable(), treeData }: Props = $props();
</script>

<Popover.Root>
	<Popover.Trigger>
		<Button variant="ghost" size="icon" aria-label="Plot settings">
			<Settings class="size-5 text-muted-foreground" />
		</Button>
	</Popover.Trigger>
	<Popover.Content class="w-[600px] p-0" onCloseAutoFocus={(e) => e.preventDefault()}>
		<Tabs.Root bind:value={plot.activeTab} class="w-full p-2 flex flex-col bg-background">
			<Tabs.List class="grid w-full grid-cols-2">
				<Tabs.Trigger value="selection">Plot Selection</Tabs.Trigger>
				<Tabs.Trigger value="settings">Plot Settings</Tabs.Trigger>
			</Tabs.List>

			<Tabs.Content value="selection" class="mt-2 flex-1 min-h-0">
				<ScrollArea class="h-full">
					<div class="p-2 h-[40vh]">
						<DataTable
							{columns}
							data={treeData}
							getSubRows={(row: TreeRow) => row.subRows}
							bind:expanded={plot.expansion}
							bind:rowSelection={plot.rowSelection}
						/>
					</div>
				</ScrollArea>
			</Tabs.Content>
			<Tabs.Content value="settings" class="mt-2 flex-1 min-h-0">
				<ScrollArea class="h-full">
					<div class="p-2 max-h-[40vh]">
						<PlotSettings bind:plot />
					</div>
				</ScrollArea>
			</Tabs.Content>
		</Tabs.Root>
	</Popover.Content>
</Popover.Root>