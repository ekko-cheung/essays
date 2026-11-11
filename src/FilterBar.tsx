import { useState } from "react";
import type { Status, Tag } from "./api";

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

interface Props {
  statuses: Status[];
  tags: Tag[];
  filters: Filters;
  search: string;
  /** Sub-notebook toggle only makes sense when a specific notebook is selected. */
  canIncludeSub: boolean;
  onFilters: (f: Filters) => void;
  onSearch: (s: string) => void;
}

const toggle = (list: string[], id: string) =>
  list.includes(id) ? list.filter((x) => x !== id) : [...list, id];

export function FilterBar({ statuses, tags, filters, search, canIncludeSub, onFilters, onSearch }: Props) {
  const [open, setOpen] = useState(false);
  const count = activeCount(filters);

  return (
    <div className="filter-bar">
      <div className="search-row">
        <input
          className="search-input"
          value={search}
          placeholder="Search notes…"
          onChange={(e) => onSearch(e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && onSearch("")}
        />
        {search && (
          <button className="icon-btn" title="Clear search" onClick={() => onSearch("")}>
            ×
          </button>
        )}
        <button
          className={`filter-toggle ${open || count ? "on" : ""}`}
          onClick={() => setOpen(!open)}
          title="Filter by status and tag"
        >
          Filter{count > 0 && <span className="badge">{count}</span>}
        </button>
      </div>

      {open && (
        <div className="filter-panel">
          <div className="filter-group">
            <div className="filter-label">
              Status <span className="hint">any of</span>
            </div>
            <div className="filter-chips">
              {statuses.map((s) => (
                <button
                  key={s.id}
                  className={`fchip ${filters.statusIds.includes(s.id) ? "on" : ""}`}
                  style={{ ["--c" as string]: s.color }}
                  onClick={() => onFilters({ ...filters, statusIds: toggle(filters.statusIds, s.id) })}
                >
                  {s.name}
                </button>
              ))}
              <button
                className={`fchip ${filters.noStatus ? "on" : ""}`}
                style={{ ["--c" as string]: "#9ca3af" }}
                onClick={() => onFilters({ ...filters, noStatus: !filters.noStatus })}
              >
                No status
              </button>
            </div>
          </div>

          <div className="filter-group">
            <div className="filter-label">
              Tags <span className="hint">all of</span>
            </div>
            <div className="filter-chips">
              {tags.map((t) => (
                <button
                  key={t.id}
                  className={`fchip ${filters.tagIds.includes(t.id) ? "on" : ""}`}
                  style={{ ["--c" as string]: t.color }}
                  onClick={() => onFilters({ ...filters, tagIds: toggle(filters.tagIds, t.id) })}
                >
                  {t.name}
                </button>
              ))}
              {tags.length === 0 && <span className="hint">No tags yet</span>}
            </div>
          </div>

          <div className="filter-foot">
            {canIncludeSub ? (
              <label>
                <input
                  type="checkbox"
                  checked={filters.includeSub}
                  onChange={(e) => onFilters({ ...filters, includeSub: e.target.checked })}
                />{" "}
                Include sub-notebooks
              </label>
            ) : (
              <span />
            )}
            {count > 0 && (
              <button
                className="link-btn"
                onClick={() => onFilters({ ...emptyFilters(filters.includeSub) })}
              >
                Clear filters
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
