import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Icon } from "../common/Icon";

export interface ConfirmOptions {
  title: string;
  message?: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  /** Destructive action: red button, and Cancel gets initial focus so Enter is safe. */
  danger?: boolean;
}

type ConfirmFn = (options: ConfirmOptions) => Promise<boolean>;

const ConfirmContext = createContext<ConfirmFn | null>(null);

/** `const confirm = useConfirm(); if (await confirm({ title: "…" })) …` */
export function useConfirm(): ConfirmFn {
  const fn = useContext(ConfirmContext);
  if (!fn) throw new Error("useConfirm must be used inside <ConfirmProvider>");
  return fn;
}

interface Pending {
  options: ConfirmOptions;
  resolve: (ok: boolean) => void;
}

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const [pending, setPending] = useState<Pending | null>(null);
  const pendingRef = useRef<Pending | null>(null);

  const confirm = useCallback<ConfirmFn>((options) => {
    // A new request supersedes an unanswered one (treated as "cancel").
    pendingRef.current?.resolve(false);
    return new Promise<boolean>((resolve) => {
      const next = { options, resolve };
      pendingRef.current = next;
      setPending(next);
    });
  }, []);

  const answer = useCallback((ok: boolean) => {
    const current = pendingRef.current;
    pendingRef.current = null;
    setPending(null);
    current?.resolve(ok);
  }, []);

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      {pending && <ConfirmDialog options={pending.options} onAnswer={answer} />}
    </ConfirmContext.Provider>
  );
}

function ConfirmDialog({
  options,
  onAnswer,
}: {
  options: ConfirmOptions;
  onAnswer: (ok: boolean) => void;
}) {
  const { title, message, danger, confirmLabel = "Confirm", cancelLabel = "Cancel" } = options;
  const cancelRef = useRef<HTMLButtonElement>(null);
  const okRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    (danger ? cancelRef : okRef).current?.focus();
  }, [danger]);

  useEffect(() => {
    // Capture phase + stopImmediatePropagation: Esc must close only this dialog,
    // not a Manage dialog (or anything else) listening underneath.
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopImmediatePropagation();
        e.preventDefault();
        onAnswer(false);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onAnswer]);

  return (
    <div className="overlay top" onMouseDown={() => onAnswer(false)}>
      <div
        className="dialog confirm"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="confirm-body">
          <div className={`confirm-icon ${danger ? "danger" : ""}`}>
            <Icon name={danger ? "alert" : "help"} size={20} />
          </div>
          <div className="confirm-text">
            <h3 id="confirm-title">{title}</h3>
            {message && <p>{message}</p>}
          </div>
        </div>
        <div className="confirm-actions">
          <button ref={cancelRef} className="btn" onClick={() => onAnswer(false)}>
            {cancelLabel}
          </button>
          <button
            ref={okRef}
            className={danger ? "btn danger" : "primary"}
            onClick={() => onAnswer(true)}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
