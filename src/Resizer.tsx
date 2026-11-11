import { useRef, useState } from "react";

interface Props {
  width: number;
  min: number;
  /** Largest width the pane may take right now (keeps the editor above its minimum). */
  max: number;
  defaultWidth: number;
  label: string;
  onChange: (w: number) => void;
}

/** Vertical drag handle on the right edge of a pane. Also works with ←/→ and double-click to reset. */
export function Resizer({ width, min, max, defaultWidth, label, onChange }: Props) {
  const start = useRef<{ x: number; w: number } | null>(null);
  const [active, setActive] = useState(false);
  const clamp = (w: number) => Math.round(Math.max(min, Math.min(w, Math.max(min, max))));

  return (
    <div
      className={`resizer${active ? " active" : ""}`}
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={width}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        start.current = { x: e.clientX, w: width };
        setActive(true);
      }}
      onPointerMove={(e) => {
        if (start.current) onChange(clamp(start.current.w + e.clientX - start.current.x));
      }}
      onPointerUp={() => {
        start.current = null;
        setActive(false);
      }}
      onLostPointerCapture={() => {
        start.current = null;
        setActive(false);
      }}
      onDoubleClick={() => onChange(clamp(defaultWidth))}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") onChange(clamp(width - 16));
        else if (e.key === "ArrowRight") onChange(clamp(width + 16));
        else return;
        e.preventDefault();
      }}
    />
  );
}
