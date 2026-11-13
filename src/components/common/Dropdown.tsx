import { useEffect, useId, useRef, useState } from "react";
import { Icon } from "./Icon";

export interface DropdownOption {
  value: string;
  label: string;
  /** Color for the leading dot; null renders a hollow dot (e.g. "No status"). */
  color?: string | null;
}

interface Props {
  value: string;
  options: DropdownOption[];
  onChange: (value: string) => void;
  ariaLabel: string;
}

function Dot({ color }: { color?: string | null }) {
  return (
    <span
      className="status-dot"
      style={{ background: color ?? "transparent", borderColor: color ?? "var(--muted)" }}
    />
  );
}

/**
 * Themed replacement for a native <select> (whose popup is OS-drawn and can't be
 * styled; on Windows it ignores the app's theme). Keyboard: Up/Down/Home/End move,
 * Enter/Space pick, Esc closes. Focus stays on the trigger (aria-activedescendant).
 */
export function Dropdown({ value, options, onChange, ariaLabel }: Props) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const id = useId();

  const selectedIndex = Math.max(0, options.findIndex((o) => o.value === value));
  const selected = options[selectedIndex];

  const openMenu = () => {
    setActive(selectedIndex);
    setOpen(true);
  };
  const pick = (i: number) => {
    setOpen(false);
    if (options[i] && options[i].value !== value) onChange(options[i].value);
  };

  // Close on outside click.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  // Keep the highlighted row visible in long lists.
  useEffect(() => {
    if (open) menu.current?.querySelector<HTMLElement>(`[data-i="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [open, active]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.nativeEvent.isComposing) return;
    switch (e.key) {
      case "ArrowDown":
      case "ArrowUp": {
        e.preventDefault();
        if (!open) return openMenu();
        const step = e.key === "ArrowDown" ? 1 : -1;
        setActive((a) => (a + step + options.length) % options.length);
        break;
      }
      case "Home":
      case "End":
        if (open) {
          e.preventDefault();
          setActive(e.key === "Home" ? 0 : options.length - 1);
        }
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        if (open) pick(active);
        else openMenu();
        break;
      case "Escape":
        if (open) {
          e.preventDefault();
          e.stopPropagation();
          setOpen(false);
        }
        break;
      case "Tab":
        setOpen(false);
        break;
    }
  };

  return (
    <div className="dd" ref={root}>
      <button
        type="button"
        className={`dd-trigger ${open ? "open" : ""}`}
        role="combobox"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={`${id}-list`}
        aria-activedescendant={open ? `${id}-${active}` : undefined}
        onClick={() => (open ? setOpen(false) : openMenu())}
        onKeyDown={onKeyDown}
      >
        <Dot color={selected?.color} />
        <span className="dd-label">{selected?.label}</span>
        <Icon name="chevron" size={12} className="dd-chevron" />
      </button>
      {open && (
        <div className="dd-menu" role="listbox" id={`${id}-list`} ref={menu}>
          {options.map((o, i) => (
            <div
              key={o.value}
              id={`${id}-${i}`}
              data-i={i}
              role="option"
              aria-selected={i === selectedIndex}
              className={`dd-item ${i === active ? "active" : ""} ${i === selectedIndex ? "selected" : ""}`}
              onMouseEnter={() => setActive(i)}
              // mousedown (not click) so it fires before the trigger could lose focus
              onMouseDown={(e) => {
                e.preventDefault();
                pick(i);
              }}
            >
              <Dot color={o.color} />
              <span className="dd-label">{o.label}</span>
              {i === selectedIndex && <Icon name="check" size={14} className="dd-check" />}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
