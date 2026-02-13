<script lang="ts">
    import "uplot/dist/uPlot.min.css";
    import "../app.css";
    import StatusFooter from "$lib/components/StatusFooter.svelte";
    import { ModeWatcher, setMode } from "mode-watcher";

    import DeviceSelectorDialog from "$lib/components/device-select/DeviceSelectorDialog.svelte";

    import { uiState } from "$lib/states/uiState.svelte";
    import { listen } from "@tauri-apps/api/event";
    import { onMount } from "svelte";
    import type { Snippet } from "svelte";
    import { Toaster } from "$lib/components/ui/sonner/index.js";

    let { children } = $props<{ children: Snippet }>();

    onMount(() => {
        uiState.open("discovery");

        let unlistenTheme: (() => void) | undefined;
        void listen<string>("theme-mode-selected", ({ payload }) => {
            if (
                payload === "system" ||
                payload === "light" ||
                payload === "dark"
            ) {
                setMode(payload);
            }
        }).then((unlisten) => {
            unlistenTheme = unlisten;
        });

        return () => {
            unlistenTheme?.();
        };
    });
</script>

<ModeWatcher />
<Toaster richColors />

<div class="flex h-screen flex-col gap-1 bg-background p-4 text-foreground">
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
