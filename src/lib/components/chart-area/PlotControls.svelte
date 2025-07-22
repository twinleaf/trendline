<script lang="ts">
import type { PlotConfig } from '$lib/states/chartState.svelte';


import { columns, type TreeRow } from '$lib/components/chart-area/data-table/column';
import DataTable from '$lib/components/chart-area/data-table/DataTable.svelte';
import PlotSettings from '$lib/components/chart-area/PlotSettings.svelte';


import { Button } from '$lib/components/ui/button/';
import * as Popover from '$lib/components/ui/popover';
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
	<Popover.Content
        class="w-[600px] p-0"
        onCloseAutoFocus={(e) => e.preventDefault()}
    >
		<Tabs.Root bind:value={plot.activeTab} class="w-full p-2">
			<Tabs.List class="grid w-full grid-cols-2">
				<Tabs.Trigger value="selection">Plot Selection</Tabs.Trigger>
				<Tabs.Trigger value="settings">Plot Settings</Tabs.Trigger>
			</Tabs.List>
			<Tabs.Content value="selection">
				<div class="flex max-h-[40vh] flex-col overflow-y-auto p-2">
					<DataTable
						{columns}
						data={treeData}
						getSubRows={(row: TreeRow) => row.subRows}
						bind:expanded={plot.expansion}
						bind:rowSelection={plot.rowSelection}
						bind:scrollTop={plot.scrollTops.selection}
					/>
				</div>
			</Tabs.Content>
			<Tabs.Content value="settings">
				<div class="flex max-h-[40vh] flex-col overflow-y-auto p-2">
					<PlotSettings 
					bind:plot 
					bind:scrollTop={plot.scrollTops.settings} 
				/>
				</div>
			</Tabs.Content>
		</Tabs.Root>
	</Popover.Content>
</Popover.Root>