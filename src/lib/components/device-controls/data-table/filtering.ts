import type { FilterFn, SortingFn } from '@tanstack/table-core';
import type { RpcMeta } from '$lib/bindings/RpcMeta';
import { rankItem, compareItems, rankings } from '@tanstack/match-sorter-utils';
import { sortingFns } from '@tanstack/table-core';

type ItemRank = ReturnType<typeof rankItem>;

type FuzzyFilterMeta = {
	itemRank: ItemRank | null;
};

export const fuzzyFilter: FilterFn<RpcMeta> = (row, columnId, value, addMeta) => {
    if (!value) {
		addMeta({ itemRank: null });
		return true;
	}
	const text = String(row.getValue(columnId) ?? '');
	const q = String(value);
	if (text.toLowerCase().includes(q.toLowerCase())) {
		addMeta({ itemRank: { rankedValue: text, rank: rankings.CASE_SENSITIVE_EQUAL, passed: true } as any });
		return true;
	}
	const itemRank = rankItem(text, q);
	addMeta({ itemRank });
	return itemRank.passed;
};

export const fuzzySort: SortingFn<RpcMeta> = (rowA, rowB, columnId) => {
	const itemRankA = (rowA.columnFiltersMeta[columnId] as FuzzyFilterMeta)?.itemRank;
	const itemRankB = (rowB.columnFiltersMeta[columnId] as FuzzyFilterMeta)?.itemRank;

	if (itemRankA && itemRankB) {
		return compareItems(itemRankA, itemRankB);
	}

	return sortingFns.alphanumeric(rowA, rowB, columnId);
};

export const prefixFilter: FilterFn<RpcMeta> = (row, columnId, value: string[]) => {
	if (!value || value.length === 0) return false;

	const key = value.join('|');
	let cached = prefixSetCache.get(key);
	if (!cached) {
		cached = new Set(value);
		prefixSetCache.set(key, cached);
	}
	const prefix = row.getValue<string>(columnId);

	return cached.has(prefix);
};

const prefixSetCache = new Map<string, Set<string>>();