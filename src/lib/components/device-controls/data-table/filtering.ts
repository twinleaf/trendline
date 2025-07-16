import type { FilterFn, SortingFn } from '@tanstack/table-core';
import type { RpcMeta } from '$lib/bindings/RpcMeta';
import { rankItem, compareItems } from '@tanstack/match-sorter-utils';
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
	const itemRank = rankItem(row.getValue(columnId), value);
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
	if (!value || value.length === 0) return true;

	// For performance, create a temporary Set from the array.
	const prefixes = new Set(value);
	const prefix = row.getValue<string>(columnId);

	return prefixes.has(prefix);
};