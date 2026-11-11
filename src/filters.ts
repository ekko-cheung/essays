/** Status / tag selection. Semantics: status = any of, tags = all of. */
export interface Filters {
  statusIds: string[];
  noStatus: boolean;
  tagIds: string[];
  includeSub: boolean;
}

export const emptyFilters = (includeSub: boolean): Filters => ({
  statusIds: [],
  noStatus: false,
  tagIds: [],
  includeSub,
});

export const activeCount = (f: Filters) =>
  f.statusIds.length + (f.noStatus ? 1 : 0) + f.tagIds.length;

export const toggleId = (list: string[], id: string) =>
  list.includes(id) ? list.filter((x) => x !== id) : [...list, id];
