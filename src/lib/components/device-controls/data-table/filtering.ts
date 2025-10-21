import type { FilterFn } from '@tanstack/table-core';
import type { RpcMeta } from '$lib/bindings/RpcMeta';
import { rankItem, rankings } from '@tanstack/match-sorter-utils';
import '@tanstack/table-core';

type ItemRank = ReturnType<typeof rankItem>;

export const fuzzyFilter: FilterFn<RpcMeta> = (row, columnId, value, addMeta) => {
	if (!value) {
		addMeta({ itemRank: null });
		return true;
	}
	const text = String(row.getValue(columnId) ?? '');
	const q = String(value);
	if (text.toLowerCase().includes(q.toLowerCase())) {
		addMeta({
			itemRank: { rankedValue: text, rank: rankings.CASE_SENSITIVE_EQUAL, passed: true } as any
		});
		return true;
	}
	const itemRank = rankItem(text, q);
	addMeta({ itemRank });
	return itemRank.passed;
};

const prefixSetCache = new Map<string, Set<string>>();

export const prefixFilter: FilterFn<RpcMeta> = (row, columnId, value: string[]) => {
  if (!value || value.length === 0) return true;

  const key = value.join('|');
  let cached = prefixSetCache.get(key);

  if (!cached) {
    cached = new Set(value);
    if (prefixSetCache.size > 50) {
      prefixSetCache.clear();
	  console.log("[DeviceControls] Cleared prefixSetCache")
    }
    prefixSetCache.set(key, cached);
  }

  const prefix = row.getValue<string>(columnId);
  return cached.has(prefix);
};