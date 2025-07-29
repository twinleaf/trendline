import type { RowSelectionState, ExpandedState } from '@tanstack/table-core';

class StreamMonitorState {
	rowSelection = $state<RowSelectionState>({});
	expansion = $state<ExpandedState>({});
}

export const streamMonitorState = new StreamMonitorState();