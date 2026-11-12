import { useEffect } from "react";
import { Icon } from "./Icon";
import { MarkdownPreview } from "./MarkdownPreview";
import type { Updater } from "./updater";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(date?: string): string | null {
  if (!date) return null;
  const parsed = new Date(date);
  return Number.isNaN(parsed.getTime()) ? null : parsed.toLocaleDateString();
}

export function UpdateDialog({ updater }: { updater: Updater }) {
  const { dialogOpen, phase, update, downloaded, total, error, dismiss } = updater;
  const busy = phase === "downloading" || phase === "installing";

  useEffect(() => {
    if (!dialogOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dialogOpen, busy, dismiss]);

  if (!dialogOpen || !update) return null;

  const percent = total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : null;
  const released = formatDate(update.date);

  return (
    <div className="overlay top" onMouseDown={() => !busy && dismiss()}>
      <div className="dialog update-dialog" onMouseDown={(e) => e.stopPropagation()}>
        <div className="dialog-head">
          <h3>
            {phase === "ready"
              ? "Update installed"
              : busy
                ? "Updating…"
                : "Update available"}
          </h3>
          {!busy && (
            <button className="icon-btn" onClick={dismiss} aria-label="Close">
              <Icon name="x" size={16} />
            </button>
          )}
        </div>

        <div className="update-body">
          <div className="update-version">
            <span className="update-old">v{update.currentVersion}</span>
            <Icon name="chevron" size={14} />
            <strong className="update-new">v{update.version}</strong>
            {released && <small>Released {released}</small>}
          </div>

          {phase === "available" && update.body && (
            <div className="update-notes">
              <MarkdownPreview source={update.body} />
            </div>
          )}

          {busy && (
            <div className="update-progress">
              <div className="update-bar">
                <div
                  className={percent === null ? "indeterminate" : ""}
                  style={percent === null ? undefined : { width: `${percent}%` }}
                />
              </div>
              <div className="update-progress-meta">
                {phase === "installing"
                  ? "Installing…"
                  : percent === null
                    ? `Downloading… ${formatBytes(downloaded)}`
                    : `${percent}% · ${formatBytes(downloaded)} of ${formatBytes(total)}`}
              </div>
            </div>
          )}

          {phase === "ready" && (
            <p className="update-ready">
              Version {update.version} is installed. Restart the app to start using it.
            </p>
          )}

          {error && (
            <div className="error" onClick={dismiss}>
              {error}
            </div>
          )}
        </div>

        <div className="confirm-actions">
          {phase === "available" && (
            <>
              <button className="btn" onClick={dismiss}>
                Later
              </button>
              <button className="primary with-icon" onClick={() => void updater.install()}>
                <Icon name="download" size={14} /> Install update
              </button>
            </>
          )}
          {busy && <span className="update-hint">Keep the app open while it updates.</span>}
          {phase === "ready" && (
            <>
              <button className="btn" onClick={dismiss}>
                Later
              </button>
              <button className="primary with-icon" onClick={() => void updater.restart()}>
                <Icon name="refresh" size={14} /> Restart now
              </button>
            </>
          )}
          {phase === "error" && (
            <>
              <button className="btn" onClick={dismiss}>
                Close
              </button>
              <button className="primary" onClick={() => void updater.install()}>
                Retry
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
