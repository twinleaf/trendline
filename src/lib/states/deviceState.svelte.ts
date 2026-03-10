import { listen } from "@tauri-apps/api/event";
import type { PortState } from "$lib/bindings/PortState";
import type { UiDevice } from "$lib/bindings/UiDevice";
import type { UiRpcError } from "$lib/bindings/UiRpcError";
import type { UiRpcMeta } from "$lib/bindings/UiRpcMeta";
import type { UiSelectionState } from "$lib/bindings/UiSelectionState";
import { uiState } from "$lib/states/uiState.svelte";
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { invoke } from "@tauri-apps/api/core";
import { sortUiDevicesByRoute, isRootRoute } from "$lib/utils";

type DeviceTreeItem = UiDevice & { children: UiDevice[] };
const LOCAL_TCP_URL_PREFIX = "tcp://localhost";

const isLocalTcpUrl = (url: string) => url.startsWith(LOCAL_TCP_URL_PREFIX);
const isSerialUrl = (url: string) => url.startsWith("serial://");
const deviceIdentityKey = (device: UiDevice) => {
  const serialNumber = device.meta.serial_number.trim();
  return serialNumber.length > 0 && serialNumber !== "N/A"
    ? `serial:${serialNumber}`
    : `url:${device.url}`;
};

function shouldPreferDevice(
  candidate: DeviceTreeItem,
  current: DeviceTreeItem,
  preferredUrl: string | null,
) {
  if (candidate.url === preferredUrl && current.url !== preferredUrl)
    return true;
  if (current.url === preferredUrl && candidate.url !== preferredUrl)
    return false;

  const candidateIsLocalTcp = isLocalTcpUrl(candidate.url);
  const currentIsLocalTcp = isLocalTcpUrl(current.url);
  if (candidateIsLocalTcp !== currentIsLocalTcp) return candidateIsLocalTcp;

  const candidateIsSerial = isSerialUrl(candidate.url);
  const currentIsSerial = isSerialUrl(current.url);
  if (candidateIsSerial !== currentIsSerial) return !candidateIsSerial;

  return candidate.url.localeCompare(current.url) < 0;
}

export interface Selection {
  portUrl: string;
  childrenRoutes: string[];
}

class DeviceState {
  #devicesMap = new SvelteMap<
    string,
    { state: PortState; devices: UiDevice[] }
  >();
  #pendingConnectUrls = new SvelteSet<string>();
  activePortUrl = $state<string | null>(null);
  selection = $state<Selection | null>(null);
  // Map<portUrl, Set<route>>
  childrenSelections = new SvelteMap<string, Set<string>>();

  constructor() {
    this.#initializeState();

    listen<[string, PortState]>(
      "port-state-changed",
      ({ payload: [url, newState] }) => {
        this.#setPortState(url, newState);
      },
    );

    listen<UiDevice[]>(
      "port-devices-discovered",
      ({ payload: new_devices }) => {
        if (new_devices.length === 0) return;

        const url = new_devices[0].url;
        console.log(
          `[DeviceState] Received batch of ${new_devices.length} devices for port ${url}`,
        );

        const entry = this.#devicesMap.get(url) ?? {
          state: new_devices[0].state,
          devices: [],
        };

        new_devices.sort(sortUiDevicesByRoute);

        this.#devicesMap.set(url, { ...entry, devices: new_devices });
        if (new_devices.some((device) => isRootRoute(device.route))) {
          this.#pendingConnectUrls.delete(url);
        }
        this.#reconcileChildrenSelection(url);
      },
    );

    listen<string>("device-removed", ({ payload: url }) => {
      this.#pendingConnectUrls.delete(url);
      this.#removePort(url);
    });

    listen<UiSelectionState>("selection-state-changed", ({ payload }) => {
      this.#applySelectionState(payload);
    });

    listen<{ portUrl: string; childrenRoutes: string[] }>(
      "device-reconnected",
      ({ payload: { portUrl } }) => {
        this.#reconcileChildrenSelection(portUrl);
      },
    );

    listen<UiDevice>(
      "device-metadata-updated",
      ({ payload: updatedDevice }) => {
        console.log(
          `[DeviceState] Metadata updated: ${updatedDevice.meta.name} @ ${updatedDevice.url}${updatedDevice.route}`,
        );
        const portUrl = updatedDevice.url;
        const portEntry = this.#devicesMap.get(portUrl);
        if (!portEntry) return;

        const deviceIndex = portEntry.devices.findIndex(
          (d) => d.route === updatedDevice.route,
        );
        if (deviceIndex > -1) {
          portEntry.devices[deviceIndex] = updatedDevice;
          this.#devicesMap.set(portUrl, { ...portEntry });
          if (isRootRoute(updatedDevice.route)) {
            this.#pendingConnectUrls.delete(portUrl);
          }
        }
      },
    );
  }

  #removePort(url: string) {
    this.#devicesMap.delete(url);
    this.childrenSelections.delete(url);
    if (this.activePortUrl === url) this.activePortUrl = null;
    if (this.selection?.portUrl === url) this.selection = null;
  }

  async #initializeState() {
    try {
      const [allCurrentDevices, selectionState] = await Promise.all([
        invoke<UiDevice[]>("get_all_devices"),
        invoke<UiSelectionState>("get_selection_state"),
      ]);
      if (allCurrentDevices.length === 0) {
        this.#applySelectionState(selectionState);
        return;
      }

      const groupedByPort = new Map<string, UiDevice[]>();
      for (const device of allCurrentDevices) {
        if (!groupedByPort.has(device.url)) groupedByPort.set(device.url, []);
        groupedByPort.get(device.url)!.push(device);
      }

      for (const [url, devicesForPort] of groupedByPort.entries()) {
        try {
          const currentState = await invoke<PortState>("get_port_state", {
            portUrl: url,
          });

          devicesForPort.sort(sortUiDevicesByRoute);
          this.#devicesMap.set(url, {
            state: currentState,
            devices: devicesForPort,
          });

          if (currentState === "Online") this.#setDefaultChildrenForPort(url);
        } catch {
          const fallbackState = devicesForPort[0]?.state ?? "Disconnected";
          devicesForPort.sort(sortUiDevicesByRoute);
          this.#devicesMap.set(url, {
            state: fallbackState,
            devices: devicesForPort,
          });
        }
      }

      this.#applySelectionState(selectionState);
    } catch (e) {
      console.error("Failed to get initial device list from backend:", e);
    }
  }

  #applySelectionState(selectionState: UiSelectionState) {
    this.activePortUrl = selectionState.active_port_url ?? null;

    if (!selectionState.confirmed) {
      this.selection = null;
      return;
    }

    const nextSelection = {
      portUrl: selectionState.confirmed.port_url,
      childrenRoutes: selectionState.confirmed.children_routes,
    };
    this.selection = nextSelection;
    this.childrenSelections.set(
      nextSelection.portUrl,
      new Set(nextSelection.childrenRoutes),
    );
  }

  #setPortState(url: string, state: PortState) {
    const entry = this.#devicesMap.get(url) ?? { state, devices: [] };
    const hasRootDevice = entry.devices.some((device) =>
      isRootRoute(device.route),
    );
    this.#devicesMap.set(url, { ...entry, state });

    const isErrorState =
      typeof state === "object" && state !== null && "Error" in state;
    if (!hasRootDevice && (state === "Disconnected" || isErrorState)) {
      this.#pendingConnectUrls.delete(url);
    }

    if (state === "Online") this.#setDefaultChildrenForPort(url);
  }

  #setDefaultChildrenForPort(url: string) {
    if (this.childrenSelections.has(url)) {
      console.log(
        `[DeviceState] Already has childrenSelection ${url} inside setDefaultChildrenForPort`,
      );
      return;
    }
    const portData = this.#devicesMap.get(url);
    if (!portData || portData.devices.length === 0) {
      console.log(
        `[DeviceState] Found no portData or devices inside setDefaultChildrenForPort`,
      );
      return;
    }
    const allChildrenRoutes = new Set(
      portData.devices.filter((d) => !isRootRoute(d.route)).map((d) => d.route),
    );
    this.childrenSelections.set(url, allChildrenRoutes);
    this.#reconcileChildrenSelection(url);
  }

  #reconcileChildrenSelection(url: string) {
    const portData = this.#devicesMap.get(url);
    if (!portData) {
      console.log(`[DeviceState] Found no portData reconcileChildrenSelection`);
      return;
    }
    const allChildrenRoutes = portData.devices
      .filter((d) => !isRootRoute(d.route))
      .map((d) => d.route);

    let set = this.childrenSelections.get(url);
    if (!set) {
      set = new Set(allChildrenRoutes);
      this.childrenSelections.set(url, set);
      console.log(
        `[DeviceState] Set childrenSelections with set of ${this.selection?.portUrl} inside reconcileChildrenSelection`,
      );
    } else {
      for (const r of allChildrenRoutes) if (!set.has(r)) set.add(r);
      for (const r of Array.from(set))
        if (!allChildrenRoutes.includes(r)) set.delete(r);
      this.childrenSelections.set(url, set);
      console.log(
        `[DeviceState] Set childrenSelections of ${this.selection?.portUrl} inside reconcileChildrenSelection`,
      );
    }
  }

  toggleChildSelection(
    portUrl: string,
    childRoute: string,
    isChecked: boolean,
  ) {
    const selections = this.childrenSelections.get(portUrl);
    if (!selections) return;

    if (isChecked) selections.add(childRoute);
    else selections.delete(childRoute);

    this.childrenSelections.set(portUrl, selections);
  }

  async connectToPort(portUrl: string) {
    this.#pendingConnectUrls.add(portUrl);
    try {
      await invoke("connect_to_port", { portUrl });
      return true;
    } catch (e) {
      this.#pendingConnectUrls.delete(portUrl);
      uiState.showError(e as string);
      return false;
    }
  }

  async refreshPort(portUrl: string) {
    try {
      await invoke("refresh_port", { portUrl });
    } catch (e) {
      uiState.showError(e as string);
    }
  }

  async disconnectPort(portUrl: string) {
    try {
      await invoke("disconnect_port", { portUrl });
    } catch (e) {
      uiState.showError(e as string);
    }
  }

  async confirmSelection(portUrl: string, childrenRoutes: string[]) {
    try {
      await invoke("confirm_selection", { portUrl, childrenRoutes });
      return true;
    } catch (e) {
      uiState.showError(e as string);
      return false;
    }
  }

  async setDiscoveryPolling(enabled: boolean) {
    try {
      await invoke("set_discovery_polling", { enabled });
    } catch (e) {
      console.error("set_discovery_polling failed", e);
    }
  }

  async executeRpc(
    device: UiDevice,
    rpc: UiRpcMeta,
    args: unknown,
  ): Promise<UiRpcError | null> {
    try {
      await invoke("execute_rpc", {
        portUrl: device.url,
        deviceRoute: device.route,
        name: rpc.name,
        args,
      });
      uiState.showSuccess(
        rpc.writable ? `Updated ${rpc.name}` : `Executed ${rpc.name}`,
      );
      return null;
    } catch (e) {
      return e as UiRpcError;
    }
  }

  devices = $derived.by(() => Array.from(this.#devicesMap.values()));

  deviceTree = $derived.by(() => {
    const placeholders: DeviceTreeItem[] = [];
    const visibleRootsByIdentity = new Map<string, DeviceTreeItem>();
    const preferredUrl = this.activePortUrl ?? this.selection?.portUrl ?? null;

    for (const [url, { state, devices }] of this.#devicesMap.entries()) {
      const parent = devices.find((d) => isRootRoute(d.route));
      const shouldRenderPlaceholder =
        !parent && this.#pendingConnectUrls.has(url);

      if (shouldRenderPlaceholder) {
        const placeholderDevice: DeviceTreeItem = {
          url,
          route: "",
          state,
          meta: {
            name: url,
            serial_number: "N/A",
            firmware_hash: "N/A",
            n_streams: 0,
            session_id: 0,
          },
          streams: [],
          rpcs: [],
          children: [],
        };
        placeholders.push(placeholderDevice);
      } else if (parent) {
        const children = devices.filter((d) => !isRootRoute(d.route));
        const rootDevice = { ...parent, state, children };
        const identityKey = deviceIdentityKey(rootDevice);
        const existing = visibleRootsByIdentity.get(identityKey);

        if (
          !existing ||
          shouldPreferDevice(rootDevice, existing, preferredUrl)
        ) {
          visibleRootsByIdentity.set(identityKey, rootDevice);
        }
      }
    }

    return [...placeholders, ...visibleRootsByIdentity.values()].sort((a, b) =>
      a.url.localeCompare(b.url),
    );
  });

  getPort(url: string) {
    return this.#devicesMap.get(url);
  }

  getDevice(route: string): UiDevice | undefined {
    const portUrl = this.selection?.portUrl;
    if (!portUrl) return undefined;
    const portData = this.#devicesMap.get(portUrl);
    if (!portData) return undefined;
    return portData.devices.find((d) => d.route === route);
  }

  selectedDevices = $derived.by(() => {
    const sel = this.selection;
    if (!sel) return [];

    const portData = this.#devicesMap.get(sel.portUrl);
    if (!portData) return [];

    const childSet = new Set(sel.childrenRoutes);
    return portData.devices.filter(
      (d) => isRootRoute(d.route) || childSet.has(d.route),
    );
  });

  selectedPortState = $derived.by((): PortState | null => {
    const sel = this.selection;
    if (!sel) return null;

    return this.#devicesMap.get(sel.portUrl)?.state ?? null;
  });
}

export const deviceState = new DeviceState();
