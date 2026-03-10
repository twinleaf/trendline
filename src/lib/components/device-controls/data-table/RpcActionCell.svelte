<script lang="ts">
    import type { UiRpcMeta } from "$lib/bindings/UiRpcMeta";
    import type { UiDevice } from "$lib/bindings/UiDevice";
    import { deviceState } from "$lib/states/deviceState.svelte";
    import { Input } from "$lib/components/ui/input";
    import { Button } from "$lib/components/ui/button";
    import { Switch } from "$lib/components/ui/switch";
    import type { UiRpcError } from "$lib/bindings/UiRpcError";
    import * as Popover from "$lib/components/ui/popover/index.js";
    import { TriangleAlert } from "@lucide/svelte";
    import { untrack } from "svelte";
    import { cn } from "$lib/utils";
    import { onDestroy } from "svelte";

    let {
        rpc,
        device,
        isSmall,
    }: { rpc: UiRpcMeta; device: UiDevice; isSmall: boolean } = $props();

    let inputValue = $state("");
    let isLoading = $state(false);
    let pendingToggleValue = $state<boolean | null>(null);
    let rpcError = $state<UiRpcError | null>(null);
    let displayedError = $state<UiRpcError | null>(null);
    let isPopoverOpen = $state(false);
    let observer: IntersectionObserver | null = null;

    let inputContainerEl = $state<HTMLDivElement | undefined>();
    let executeButtonContainerEl = $state<HTMLDivElement | undefined>();

    $effect(() => {
        if (!isPopoverOpen) {
            if (observer) {
                observer.disconnect();
                observer = null;
            }
            return;
        }
        const el = rpc.writable ? inputContainerEl : executeButtonContainerEl;
        if (!el) return;

        observer = new IntersectionObserver(
            (entries) => {
                const entry = entries[0];
                if (!entry.isIntersecting) rpcError = null;
            },
            { threshold: 0 },
        );

        observer.observe(el);

        return () => {
            if (observer) {
                observer.disconnect();
                observer = null;
            }
        };
    });

    $effect(() => {
        isPopoverOpen = !!rpcError;
    });

    $effect(() => {
        if (rpcError) {
            displayedError = rpcError;
        }
    });

    $effect(() => {
        if (rpcError) {
            const timerId = setTimeout(() => {
                rpcError = null;
            }, 3000);
            return () => clearTimeout(timerId);
        }
    });

    $effect(() => {
        if (!isPopoverOpen) {
            untrack(() => {
                if (rpcError) rpcError = null;
            });
        }
    });

    $effect(() => {
        if (rpcError && inputContainerEl) {
            inputContainerEl
                .querySelector<HTMLElement>('input, button, [role="switch"]')
                ?.focus();
        }
    });

    $effect(() => {
        if (pendingToggleValue === null) return;
        if (rpcBooleanValue(rpc.value) === pendingToggleValue) {
            pendingToggleValue = null;
        }
    });

    function formatRpcError(err: UiRpcError | null): string {
        if (!err) return "";
        switch (err.type) {
            case "ExecError":
                const code = err.payload.error;
                let formattedCode: string;

                if (typeof code === "string") {
                    formattedCode = code.replace(/([A-Z])/g, " $1").trim();
                } else {
                    formattedCode = `Unknown (${code.Unknown})`;
                }
                return `Execution Error: ${formattedCode}`;

            case "SendFailed":
                return `Send Failed: ${err.payload}`;
            case "RecvFailed":
                return `Receive Failed: ${err.payload}`;
            case "AppLogic":
                return err.payload;
            case "TypeError":
                return "Type Error: The provided value has the wrong type.";
            default:
                return "An unknown error occurred.";
        }
    }

    async function executeRpc(args: any) {
        if (isLoading) return;
        isLoading = true;
        rpcError = null;
        try {
            const error = await deviceState.executeRpc(device, rpc, args);
            if (error) {
                pendingToggleValue = null;
                rpcError = error;
                return;
            }

            if (showsToggle) {
                return;
            }

            if (rpc.writable) {
                inputValue = "";
            }
        } catch (err) {
            pendingToggleValue = null;
            rpcError = err as UiRpcError;
        } finally {
            isLoading = false;
        }
    }

    function handleSet() {
        if (isLoading) return;
        if (
            inputValue === undefined ||
            inputValue === null ||
            inputValue === ""
        )
            return;
        let arg: any = inputValue;
        if (
            rpc.arg_type.includes("f") ||
            rpc.arg_type.includes("u") ||
            rpc.arg_type.includes("i")
        ) {
            arg = Number(inputValue);
            if (isNaN(arg)) {
                rpcError = {
                    type: "AppLogic",
                    payload: "Invalid number format.",
                };
                return;
            }
        }
        executeRpc(arg);
    }

    function handleKeyDown(event: KeyboardEvent) {
        if (
            event.key === "Enter" &&
            !event.shiftKey &&
            !event.ctrlKey &&
            !event.altKey &&
            !event.metaKey
        ) {
            event.preventDefault();
            event.stopPropagation();
            handleSet();
        }
    }

    function handleAction() {
        if (isLoading) return;
        executeRpc(null);
    }

    function isBooleanKeywordRpc(name: string): boolean {
        return name.endsWith(".enable") || name.endsWith(".manual");
    }

    function rpcBooleanValue(value: unknown): boolean | null {
        if (typeof value === "boolean") return value;
        if (typeof value === "number") {
            if (value === 0) return false;
            if (value === 1) return true;
        }
        return null;
    }

    const showsToggle = $derived.by(
        () =>
            rpc.writable &&
            rpc.readable &&
            isBooleanKeywordRpc(rpc.name) &&
            (rpc.value == null || rpcBooleanValue(rpc.value) !== null),
    );

    const currentToggleValue = $derived.by(() => {
        if (pendingToggleValue !== null) return pendingToggleValue;
        return rpcBooleanValue(rpc.value) ?? false;
    });

    function handleToggleChange(checked: boolean) {
        if (isLoading) return;
        pendingToggleValue = checked;
        executeRpc(checked ? 1 : 0);
    }

    onDestroy(() => {
        if (observer) {
            observer.disconnect();
            observer = null;
        }
    });
</script>

{#if !rpc.readable && !rpc.writable}
    <Popover.Root bind:open={isPopoverOpen}>
        <div class="flex justify-end" bind:this={executeButtonContainerEl}>
            <Button
                variant="default"
                size="sm"
                class={cn("h-8 justify-center", {
                    "w-full": isSmall,
                    "w-20": !isSmall,
                })}
                onclick={handleAction}
                disabled={isLoading}
            >
                {isLoading ? "..." : "Execute"}
            </Button>
        </div>
        <Popover.Content
            customAnchor={executeButtonContainerEl}
            side="top"
            sideOffset={4}
            class="z-10 w-auto rounded-md bg-destructive p-2 text-sm text-destructive-foreground"
            onOpenAutoFocus={(e) => e.preventDefault()}
        >
            <div class="flex items-center gap-2">
                <TriangleAlert class="size-4" />
                <p>{formatRpcError(displayedError)}</p>
            </div>
        </Popover.Content>
    </Popover.Root>
{:else if showsToggle}
    <Popover.Root bind:open={isPopoverOpen}>
        <div
            bind:this={inputContainerEl}
            class="flex h-8 w-full min-w-0 items-center justify-between gap-3 rounded-md border border-input bg-transparent px-3"
        >
            <span class="truncate font-mono text-xs text-muted-foreground">
                {currentToggleValue ? "On" : "Off"}
            </span>
            <Switch
                checked={currentToggleValue}
                disabled={isLoading || (rpc.readable && rpc.value == null)}
                aria-label={`Toggle ${rpc.name}`}
                onCheckedChange={(checked) => handleToggleChange(!!checked)}
            />
        </div>
        <Popover.Content
            customAnchor={inputContainerEl}
            side="top"
            sideOffset={4}
            class="z-10 w-auto rounded-md bg-destructive p-2 text-sm text-destructive-foreground"
            onOpenAutoFocus={(e) => e.preventDefault()}
        >
            <div class="flex items-center gap-2">
                <TriangleAlert class="size-4" />
                <p>{formatRpcError(displayedError)}</p>
            </div>
        </Popover.Content>
    </Popover.Root>
{:else if rpc.writable}
    <Popover.Root bind:open={isPopoverOpen}>
        <div class="flex w-full min-w-0 items-center gap-2">
            <div bind:this={inputContainerEl} class="flex-1">
                <Input
                    type={rpc.arg_type.includes("f") ||
                    rpc.arg_type.includes("i") ||
                    rpc.arg_type.includes("u")
                        ? "number"
                        : "text"}
                    class="h-8 w-full bg-transparent font-mono text-xs"
                    placeholder={rpc.value == null
                        ? "N/A"
                        : typeof rpc.value === "string" ||
                            typeof rpc.value === "number" ||
                            typeof rpc.value === "boolean"
                          ? String(rpc.value)
                          : "[object]"}
                    bind:value={inputValue}
                    onkeydown={handleKeyDown}
                    oninput={() => (rpcError = null)}
                    onblur={() => (rpcError = null)}
                    aria-invalid={!!rpcError}
                    readonly={isLoading}
                    autocomplete="off"
                    autocorrect="off"
                    autocapitalize="off"
                    spellcheck={false}
                    inputmode={rpc.arg_type.includes("f")
                        ? "decimal"
                        : rpc.arg_type.includes("i") ||
                            rpc.arg_type.includes("u")
                          ? "numeric"
                          : "text"}
                />
            </div>

            {#if !isSmall}
                <Button
                    variant="default"
                    size="sm"
                    class="h-8 w-20 justify-center"
                    onclick={handleSet}
                    disabled={isLoading}
                >
                    {isLoading ? "..." : "Set"}
                </Button>
            {/if}
        </div>
        <Popover.Content
            customAnchor={inputContainerEl}
            side="top"
            sideOffset={4}
            class="z-10 w-auto rounded-md bg-destructive p-2 text-sm text-destructive-foreground"
            onOpenAutoFocus={(e) => e.preventDefault()}
        >
            <div class="flex items-center gap-2">
                <TriangleAlert class="size-4" />
                <p>{formatRpcError(displayedError)}</p>
            </div>
        </Popover.Content>
    </Popover.Root>
{:else}
    <div
        class="flex h-8 min-w-0 items-center rounded-md border border-input bg-transparent px-3 py-2 text-xs ring-offset-background"
    >
        <span
            class="font-mono text-muted-foreground truncate"
            title={JSON.stringify(rpc.value)}
        >
            {JSON.stringify(rpc.value) ?? "N/A"}
        </span>
    </div>
{/if}
