import type { Status, Tag } from "../../api";
import { Icon } from "../common/Icon";
import { activeCount, emptyFilters, toggleId, type Filters } from "../../lib/filters";

interface Props {
  statuses: Status[];
  tags: Tag[];
  filters: Filters;
  onFilters: (f: Filters) => void;
  onManage: (kind: "status" | "tag") => void;
}

interface RowProps {
  color: string | null;
  name: string;
  count?: number;
  on: boolean;
  onClick: () => void;
  icon?: "tag";
}

function FilterRow({ color, name, count, on, onClick, icon }: RowProps) {
  return (
    <div className={`tree-row filter-row ${on ? "selected" : ""}`} onClick={onClick} role="checkbox" aria-checked={on}>
      {icon === "tag" ? (
        <Icon name="tag" size={14} style={{ color: color ?? undefined }} />
      ) : (
        <span
          className="status-dot"
          style={{ background: color ?? "transparent", borderColor: color ?? "var(--muted)" }}
        />
      )}
      <span className="name">{name}</span>
      {count !== undefined && <span className="row-count">{count}</span>}
    </div>
  );
}

export function SidebarFilters({ statuses, tags, filters, onFilters, onManage }: Props) {
  const active = activeCount(filters);
  return (
    <>
      <div className="side-section">
        <div className="tree-header" title="Notes with any of the selected statuses">
          <span>Status</span>
          <span className="header-actions">
            {active > 0 && (
              <button className="link-btn" onClick={() => onFilters(emptyFilters(filters.includeSub))}>
                Clear
              </button>
            )}
            <button className="icon-btn" title="Manage statuses" onClick={() => onManage("status")}>
              <Icon name="sliders" size={14} />
            </button>
          </span>
        </div>
        {statuses.map((s) => (
          <FilterRow
            key={s.id}
            color={s.color}
            name={s.name}
            count={s.noteCount}
            on={filters.statusIds.includes(s.id)}
            onClick={() => onFilters({ ...filters, statusIds: toggleId(filters.statusIds, s.id) })}
          />
        ))}
        <FilterRow
          color={null}
          name="No status"
          on={filters.noStatus}
          onClick={() => onFilters({ ...filters, noStatus: !filters.noStatus })}
        />
      </div>

      <div className="side-section">
        <div className="tree-header" title="Notes carrying all of the selected tags">
          <span>Tags</span>
          <span className="header-actions">
            <button className="icon-btn" title="Manage tags" onClick={() => onManage("tag")}>
              <Icon name="sliders" size={14} />
            </button>
          </span>
        </div>
        {tags.map((t) => (
          <FilterRow
            key={t.id}
            icon="tag"
            color={t.color}
            name={t.name}
            count={t.noteCount}
            on={filters.tagIds.includes(t.id)}
            onClick={() => onFilters({ ...filters, tagIds: toggleId(filters.tagIds, t.id) })}
          />
        ))}
        {tags.length === 0 && <div className="empty">No tags yet.</div>}
      </div>
    </>
  );
}
