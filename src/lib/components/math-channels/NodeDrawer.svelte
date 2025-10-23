<script lang="ts">
  import * as Collapsible from "$lib/components/ui/collapsible/index.js";
  import { buttonVariants } from "$lib/components/ui/button/index.js";
  import { ChevronsUpDown } from "@lucide/svelte";
  import { useDnD } from "./DnDProvider.svelte";

  const type = useDnD();

  const onDragStart = (event: DragEvent, nodeType: string) => {
    event.stopPropagation();
    if (!event.dataTransfer) return;

    type.current = nodeType;
    event.dataTransfer.effectAllowed = "move";
  };

</script>

<!-- Make sure this area stays interactive even if the parent Panel disables pointer events -->
<div class="pointer-events-auto">
  <Collapsible.Root>
    <div class="flex justify-center">
      <Collapsible.Trigger
        class={`${buttonVariants({ variant: "secondary", size: "sm" })} rounded-full shadow-md`}
        aria-label="Toggle node palette"
      >
        <ChevronsUpDown class="mr-2 h-4 w-4" />
        Nodes
      </Collapsible.Trigger>
    </div>

  <Collapsible.Content class="mt-2 flex justify-center">
      <div
        class="rounded-xl border bg-popover text-popover-foreground shadow-lg
               backdrop-blur p-3"
      >
        <div class="grid grid-cols-[auto_auto_auto] gap-4">
          <!-- Sources -->
          <section class="flex flex-col items-center">
            <h4 class="text-[11px] uppercase tracking-wide opacity-70 mb-1">Sources</h4>
            <div class="flex flex-wrap gap-2 justify-center">
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="select-none cursor-grab rounded border px-3 py-1 text-xs font-medium hover:bg-muted"
                draggable={true}
                ondragstart={(e) => onDragStart(e, "input")}
              >
                Input
              </div>
            </div>
          </section>

          <!-- Transforms -->
          <section class="flex flex-col items-center">
            <h4 class="text-[11px] uppercase tracking-wide opacity-70 mb-1">Transforms</h4>
            <div class="flex flex-wrap gap-2 justify-center">
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="select-none cursor-grab rounded border px-3 py-1 text-xs font-medium hover:bg-muted"
                draggable={true}
                ondragstart={(e) => onDragStart(e, "default")}
              >
                Default
              </div>
            </div>
          </section>

          <!-- Sinks -->
          <section class="flex flex-col items-center">
            <h4 class="text-[11px] uppercase tracking-wide opacity-70 mb-1">Sinks</h4>
            <div class="flex flex-wrap gap-2 justify-center">
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="select-none cursor-grab rounded border px-3 py-1 text-xs font-medium hover:bg-muted"
                draggable={true}
                ondragstart={(e) => onDragStart(e, "output")}
              >
                Output
              </div>
            </div>
          </section>
        </div>
      </div>
    </Collapsible.Content>
  </Collapsible.Root>
</div>
