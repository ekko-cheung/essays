interface Props {
  search: string;
  onSearch: (s: string) => void;
  /** Sub-notebook toggle only makes sense when a specific notebook is selected. */
  canIncludeSub: boolean;
  includeSub: boolean;
  onIncludeSub: (v: boolean) => void;
}

export function SearchBar({ search, onSearch, canIncludeSub, includeSub, onIncludeSub }: Props) {
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
      </div>
      {canIncludeSub && (
        <label className="sub-toggle">
          <input type="checkbox" checked={includeSub} onChange={(e) => onIncludeSub(e.target.checked)} />{" "}
          Include sub-notebooks
        </label>
      )}
    </div>
  );
}
