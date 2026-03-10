import type { UiColumnKey } from "$lib/bindings/UiColumnKey";
import type { MathChannelConfig } from "$lib/bindings/MathChannelConfig";
import type { PipelineId } from "$lib/bindings/PipelineId";

class MathState {
  channels = $state(
    new Map<
      string,
      { key: UiColumnKey; pid: PipelineId; spec: MathChannelConfig }
    >(),
  );
  register(entry: {
    key: UiColumnKey;
    pid: PipelineId;
    spec: MathChannelConfig;
  }) {
    this.channels.set(JSON.stringify(entry.key), entry);
  }
  unregister(key: UiColumnKey) {
    this.channels.delete(JSON.stringify(key));
  }
  get(key: UiColumnKey) {
    return this.channels.get(JSON.stringify(key));
  }
}
export const mathState = new MathState();
