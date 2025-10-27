import type { DataColumnId } from '$lib/bindings/DataColumnId';
import type { MathChannelConfig } from '$lib/bindings/MathChannelConfig';
import type { PipelineId } from '$lib/bindings/PipelineId';

class MathState {
  channels = $state(new Map<string, { key: DataColumnId; pid: PipelineId; spec: MathChannelConfig }>());
  register(entry: { key: DataColumnId; pid: PipelineId; spec: MathChannelConfig }) {
    this.channels.set(JSON.stringify(entry.key), entry);
  }
  unregister(key: DataColumnId) {
    this.channels.delete(JSON.stringify(key));
  }
  get(key: DataColumnId) { return this.channels.get(JSON.stringify(key)); }
}
export const mathState = new MathState();
