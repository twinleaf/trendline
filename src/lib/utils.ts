import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import type { UiDevice } from '$lib/bindings/UiDevice';
import type { DataColumnId } from '$lib/bindings/DataColumnId';

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export const isRootRoute = (route: string) => route === '/' || route === '';

export const deviceDomId = (portUrl: string, route: string) =>
  `${portUrl}::${isRootRoute(route) ? '/' : route}`;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };


/**
 * Sorts UiDevice objects based on their route. Handles numeric sorting for
 * routes like '/1' vs '/10' and places the root device ('/' or '') first.
 * @param a The first UiDevice to compare.
 * @param b The second UiDevice to compare.
 * @returns A number indicating the sort order.
 */
export function sortUiDevicesByRoute(a: UiDevice, b: UiDevice): number {
    const routeA = a.route;
    const routeB = b.route;

    const isRootA = isRootRoute(routeA);   // <— reused here
    if (isRootA) return -1;
    const isRootB = isRootRoute(routeB);
    if (isRootB) return 1;

    // Attempt to sort numerically if routes are slash-delimited numbers
    const partsA = routeA.substring(1).split('/').map(Number);
    const partsB = routeB.substring(1).split('/').map(Number);

    if (partsA.some(isNaN) || partsB.some(isNaN)) {
      return routeA.localeCompare(routeB);
    }

    const minLength = Math.min(partsA.length, partsB.length);
    for (let i = 0; i < minLength; i++) {
      const diff = partsA[i] - partsB[i];
      if (diff !== 0) return diff;
    }
    return partsA.length - partsB.length;
}

export function lerp(x1: number, y1: number, x2: number, y2: number, x: number): number {
    if (Math.abs(x2 - x1) < 1e-9) return y1; // Avoid division by zero
    return y1 + (y2 - y1) * (x - x1) / (x2 - x1);
}

export function findTimestampIndex(timestamps: Float64Array | number[], targetTime: number): number {
    let low = 0;
    let high = timestamps.length;
    while (low < high) {
        const mid = (low + high) >>> 1; // Unsigned bitwise shift for integer division
        if (timestamps[mid] < targetTime) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    return low;
}

export const isVirtualUrl = (url: string) => url.startsWith('virtual://');
export const isVirtualKey = (k: DataColumnId) => isVirtualUrl(k.port_url);

export type ChannelCaps = {
  // high-level intent flags used by different views
  showInDeviceSelector: boolean;
  showInRpcPanel: boolean;
  showInStreamHealth: boolean;
  plottable: boolean;
  badge?: 'virtual';
};

export function capsForDevice(d: UiDevice): ChannelCaps {
  const v = isVirtualUrl(d.url);
  const hasRpcs = (d.rpcs?.length ?? 0) > 0;
  return {
    showInDeviceSelector: !v,    // Device picker should not list virtual
    showInRpcPanel: !v && hasRpcs,
    showInStreamHealth: !v,      // default policy: exclude virtual
    plottable: true,
    badge: v ? 'virtual' : undefined,
  };
}

// When you only have a column key (e.g., tables, monitors):
export function capsForKey(k: DataColumnId): ChannelCaps {
  const v = isVirtualKey(k);
  return {
    showInDeviceSelector: !v,
    showInRpcPanel: !v,
    showInStreamHealth: !v,
    plottable: true,
    badge: v ? 'virtual' : undefined,
  };
}