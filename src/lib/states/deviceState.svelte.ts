import { listen } from "@tauri-apps/api/event";
import type { PortState } from "$lib/bindings/PortState";
import type { UiDevice } from "$lib/bindings/UiDevice";
import { SvelteMap } from "svelte/reactivity";
import { invoke } from "@tauri-apps/api/core";
import { sortUiDevicesByRoute, isRootRoute } from "$lib/utils";

type DeviceTreeItem = UiDevice & { children: UiDevice[] };
const LOCAL_TCP_URL_PREFIX = "tcp://localhost";

export interface Selection {
  portUrl: string;
  childrenRoutes: string[];
}

class DeviceState {
  #devicesMap = new SvelteMap<
    string,
    { state: PortState; devices: UiDevice[] }
  >();
  selection = $state<Selection | null>(null);
  // Map<portUrl, Set<route>>
  childrenSelections = new SvelteMap<string, Set<string>>();

  constructor() {
    this.#initializeState();

    listen<[string, PortState]>(
      "port-state-changed",
      ({ payload: [url, newState] }) => {
        console.log(
          `[DeviceState] Received state of ${newState} devices for port ${url} with selection ${this.selection?.portUrl}`,
        );
        this.#setPortState(url, newState);
        console.log(
          `[DeviceState] Set state of ${newState} devices for port ${url} with selection ${this.selection?.portUrl}`,
        );
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
        this.#reconcileChildrenSelection(url);
      },
    );

    listen<string>("device-removed", ({ payload: url }) => {
      this.#removePort(url);
    });

    listen<{ portUrl: string; childrenRoutes: string[] }>(
      "device-reconnected",
      ({ payload: { portUrl, childrenRoutes } }) => {
        const set = new Set(childrenRoutes);
        deviceState.childrenSelections.set(portUrl, set);

        if (
          !deviceState.selection ||
          deviceState.selection.portUrl === portUrl
        ) {
          deviceState.selection = { portUrl, childrenRoutes: Array.from(set) };
        }

        this.#reconcileChildrenSelection?.(portUrl);
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
        }
      },
    );
  }

  #removePort(url: string) {
    this.#devicesMap.delete(url);
    this.childrenSelections.delete(url);
    if (this.selection?.portUrl === url) this.selection = null;
  }

  async #initializeState() {
    try {
      const allCurrentDevices = await invoke<UiDevice[]>("get_all_devices");
      if (allCurrentDevices.length === 0) return;

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

          if (currentState === "Streaming")
            this.#setDefaultChildrenForPort(url);
        } catch {
          const fallbackState = devicesForPort[0]?.state ?? "Disconnected";
          devicesForPort.sort(sortUiDevicesByRoute);
          this.#devicesMap.set(url, {
            state: fallbackState,
            devices: devicesForPort,
          });
        }
      }
    } catch (e) {
      console.error("Failed to get initial device list from backend:", e);
    }
  }

  #setPortState(url: string, state: PortState) {
    const entry = this.#devicesMap.get(url) ?? { state, devices: [] };
    const prev = entry.state;
    this.#devicesMap.set(url, { ...entry, state });

    if (state === "Streaming") this.#setDefaultChildrenForPort(url);
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

    if (!this.selection) {
      this.selection = {
        portUrl: url,
        childrenRoutes: Array.from(allChildrenRoutes),
      };
      console.log(
        `[DeviceState] Set ${this.selection?.portUrl} inside setDefaultChildrenForPort`,
      );
    }
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

    if (this.selection?.portUrl === url) {
      const cur = new Set(this.selection.childrenRoutes);
      for (const r of Array.from(cur)) if (!set!.has(r)) cur.delete(r);
      for (const r of set!) if (!cur.has(r)) cur.add(r);
      this.selection = { portUrl: url, childrenRoutes: Array.from(cur) };
      console.log(
        `[DeviceState] Set ${this.selection?.portUrl} inside reconcileChildrenSelection`,
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

  devices = $derived.by(() => Array.from(this.#devicesMap.values()));

  deviceTree = $derived.by(() => {
    const out: DeviceTreeItem[] = [];

    for (const [url, { state, devices }] of this.#devicesMap.entries()) {
      const parent = devices.find((d) => isRootRoute(d.route));
      const isErrorState =
        typeof state === "object" && state !== null && "Error" in state;
      const isLocalTcpUrl = url.startsWith(LOCAL_TCP_URL_PREFIX);
      const isTransitionState =
        state === "Connecting" ||
        state === "Discovery" ||
        state === "Reconnecting" ||
        state === "Disconnected" ||
        isErrorState;
      const isMissingRootWhileActive = !parent && state !== "Idle";
      const shouldRenderPlaceholder =
        !isLocalTcpUrl && (isTransitionState || isMissingRootWhileActive);

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
        out.push(placeholderDevice);
      } else if (parent) {
        const children = devices.filter((d) => !isRootRoute(d.route));
        out.push({ ...parent, state, children });
      }
    }
    return out.sort((a, b) => a.url.localeCompare(b.url));
  });

  getPort(url: string) {
    return this.#devicesMap.get(url);
  }

  getDevice(portUrl: string, route: string): UiDevice | undefined {
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
