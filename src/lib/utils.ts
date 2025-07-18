import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import type { AlignedData } from 'uplot';
import type { UiDevice } from '$lib/bindings/UiDevice';

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

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

    const isRootA = routeA === '/' || routeA === '';
    if (isRootA) return -1;
    const isRootB = routeB === '/' || routeB === '';
    if (isRootB) return 1;

    // Attempt to sort numerically if routes are slash-delimited numbers
    const partsA = routeA.substring(1).split('/').map(Number);
    const partsB = routeB.substring(1).split('/').map(Number);

    if (partsA.some(isNaN) || partsB.some(isNaN)) {
        return routeA.localeCompare(routeB); // Fallback to string compare
    }

    const minLength = Math.min(partsA.length, partsB.length);
    for (let i = 0; i < minLength; i++) {
        const diff = partsA[i] - partsB[i];
        if (diff !== 0) {
            return diff;
        }
    }

    return partsA.length - partsB.length;
}

/**
 * A Ring Buffer specifically designed for uPlot time-series data.
 * It stores a fixed number of data points and efficiently overwrites the oldest
 * points once the buffer's capacity is reached.
 */
export class TimeSeriesRingBuffer {
  private capacity: number;
  private numSeries: number;
  private head: number = 0; // Points to the next slot to be written
  private size: number = 0; // Number of elements currently in the buffer

  // Data is stored in uPlot's expected format: [timestamps, series1, series2, ...]
  private buffer: Float64Array[];
  private lastSeenTimestamp: number = -Infinity;

  constructor(capacity: number, numSeries: number) {
    if (capacity <= 0 || numSeries < 0) {
      throw new Error("Capacity and numSeries must be positive.");
    }
    this.capacity = capacity;
    this.numSeries = numSeries;

    // Allocate all arrays upfront. +1 for the timestamps array.
    this.buffer = Array.from({ length: numSeries + 1 }, () => new Float64Array(capacity));
  }

  /**
   * Appends a chunk of new data. `newData` should be in uPlot's AlignedData format.
   * This is the primary method for adding delta updates.
   * @param newData The new data to append, as [timestamps, series1, ...]
   */
  public appendBulk(newData: AlignedData): void {
    const newTimestamps = newData[0];
    const newPointsCount = newTimestamps.length;

    if (newPointsCount === 0) return;

    for (let i = 0; i < newPointsCount; i++) {
      for (let j = 0; j < this.numSeries + 1; j++) {
		const value = newData[j]?.[i];
        this.buffer[j][this.head] = value ?? NaN;
      }

      this.head = (this.head + 1) % this.capacity;

      if (this.size < this.capacity) {
        this.size++;
      }
    }

    this.lastSeenTimestamp = newTimestamps[newPointsCount - 1];
  }

  /**
   * Retrieves the latest timestamp seen by the buffer.
   * Used for the `since_timestamp` parameter in `get_decimated_delta`.
   */
  public getLastTimestamp(): number {
    return this.lastSeenTimestamp;
  }

  /**
   * Returns the data in a sorted, contiguous format suitable for uPlot.
   * This is necessary because the ring buffer's internal storage is not
   * guaranteed to be in chronological order after the head wraps around.
   */
  public getAlignedData(): Float64Array[] {
    const alignedData: Float64Array[] = Array.from(
      { length: this.numSeries + 1 },
      () => new Float64Array(this.size)
    );

    if (this.size === 0) {
      return alignedData;
    }

    // If the buffer hasn't wrapped, the data is from index 0 to `size - 1`.
    // If it has wrapped, the oldest data starts at `head`.
    const start = this.size < this.capacity ? 0 : this.head;
    const end = this.head;

    if (start < end) { // No wrap-around yet or buffer not full
      for (let i = 0; i < this.numSeries + 1; i++) {
        alignedData[i].set(this.buffer[i].subarray(start, end));
      }
    } else { // Data has wrapped around
      for (let i = 0; i < this.numSeries + 1; i++) {
        // Part 1: from the start index to the end of the array
        const tail = this.buffer[i].subarray(start);
        alignedData[i].set(tail, 0);

        // Part 2: from the beginning of the array to the end index (head)
        const headPart = this.buffer[i].subarray(0, end);
        alignedData[i].set(headPart, tail.length);
      }
    }
    return alignedData;
  }
}