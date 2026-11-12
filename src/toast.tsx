import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Icon } from "./Icon";

type ToastKind = "success" | "error";

interface ToastItem {
  id: number;
  kind: ToastKind;
  title: string;
  detail?: string;
}

interface ToastApi {
  success: (title: string, detail?: string) => void;
  error: (title: string, detail?: string) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

/** `const toast = useToast(); toast.success("Saved"); toast.error("Failed", reason);` */
export function useToast(): ToastApi {
  const api = useContext(ToastContext);
  if (!api) throw new Error("useToast must be used inside <ToastProvider>");
  return api;
}

// Errors stay longer: people need time to read the reason.
const DURATION_MS: Record<ToastKind, number> = { success: 3500, error: 9000 };
const MAX_VISIBLE = 4;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const nextId = useRef(1);

  const dismiss = useCallback((id: number) => setItems((l) => l.filter((t) => t.id !== id)), []);

  const push = useCallback((kind: ToastKind, title: string, detail?: string) => {
    const id = nextId.current++;
    setItems((l) => [...l, { id, kind, title, detail }].slice(-MAX_VISIBLE));
  }, []);

  const api = useMemo<ToastApi>(
    () => ({
      success: (title, detail) => push("success", title, detail),
      error: (title, detail) => push("error", title, detail),
    }),
    [push],
  );

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div className="toasts" aria-live="polite">
        {items.map((t) => (
          <Toast key={t.id} item={t} onDismiss={dismiss} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

function Toast({ item, onDismiss }: { item: ToastItem; onDismiss: (id: number) => void }) {
  const [hovered, setHovered] = useState(false);

  // The timer restarts after the pointer leaves, so a toast can't vanish while being read.
  useEffect(() => {
    if (hovered) return;
    const t = window.setTimeout(() => onDismiss(item.id), DURATION_MS[item.kind]);
    return () => window.clearTimeout(t);
  }, [hovered, item.id, item.kind, onDismiss]);

  return (
    <div
      className={`toast ${item.kind}`}
      role={item.kind === "error" ? "alert" : "status"}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <Icon name={item.kind === "error" ? "alert" : "check"} size={18} className="toast-icon" />
      <div className="toast-text">
        <div className="toast-title">{item.title}</div>
        {item.detail && <div className="toast-detail">{item.detail}</div>}
      </div>
      <button className="icon-btn" title="Dismiss" aria-label="Dismiss" onClick={() => onDismiss(item.id)}>
        <Icon name="x" size={14} />
      </button>
    </div>
  );
}
