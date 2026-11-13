import { useEffect, useState } from "react";
import { api, type CloseAction, type Settings, type SyncStatus } from "../../api";
import { Icon } from "../common/Icon";
import { getThemePref, setThemePref, type ThemePref } from "../../lib/theme";
import type { Updater } from "../../hooks/useUpdater";

interface Props {
  settings: Settings;
  syncing: boolean;
  updater: Updater;
  onSaved: (s: Settings) => void;
  onSyncNow: () => Promise<void>;
  onClose: () => void;
}

const formatTime = (ms: number) => new Date(ms).toLocaleString();

export function SettingsDialog({ settings, syncing, updater, onSaved, onSyncNow, onClose }: Props) {
  const [theme, setTheme] = useState<ThemePref>(getThemePref);
  const [closeAction, setCloseAction] = useState<CloseAction>(settings.closeAction);
  const [syncEnabled, setSyncEnabled] = useState(settings.syncEnabled);
  const [accountId, setAccountId] = useState(settings.accountId);
  const [databaseId, setDatabaseId] = useState(settings.databaseId);
  const [token, setToken] = useState("");
  const [autoSync, setAutoSync] = useState(settings.autoSync);
  const [interval, setIntervalMin] = useState(settings.intervalMinutes);

  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const refreshStatus = () => api.syncStatus().then(setStatus).catch(() => setStatus(null));
  useEffect(() => {
    void refreshStatus();
  }, [syncing]);

  const save = async () => {
    const saved = await api.saveSettings({
      closeAction,
      syncEnabled,
      accountId,
      databaseId,
      apiToken: token.trim() === "" ? null : token,
      autoSync,
      intervalMinutes: Math.max(1, Math.floor(interval) || 5),
    });
    setToken("");
    onSaved(saved);
    return saved;
  };

  const guard = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const credentialsFilled = accountId.trim() && databaseId.trim() && (token.trim() || settings.hasToken);

  return (
    <div className="overlay" onMouseDown={onClose}>
      <div className="dialog settings" onMouseDown={(e) => e.stopPropagation()}>
        <div className="dialog-head">
          <h3>Settings</h3>
          <button className="icon-btn" onClick={onClose} aria-label="Close">
            <Icon name="x" size={16} />
          </button>
        </div>
        {error && (
          <div className="error" onClick={() => setError(null)}>
            {error}
          </div>
        )}

        <div className="settings-body">
          <section className="set-section">
            <h4>General</h4>
            <div className="field">
              <label>Appearance</label>
              <select
                value={theme}
                onChange={(e) => {
                  const t = e.target.value as ThemePref;
                  setTheme(t);
                  setThemePref(t);
                }}
              >
                <option value="system">Follow system</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </div>
            <div className="field">
              <label>When closing the window</label>
              <div className="radio-group">
                <label className="radio">
                  <input
                    type="radio"
                    name="close"
                    checked={closeAction === "quit"}
                    onChange={() => setCloseAction("quit")}
                  />
                  <span>
                    Quit the app
                    <small>Closing the window exits completely.</small>
                  </span>
                </label>
                <label className="radio">
                  <input
                    type="radio"
                    name="close"
                    checked={closeAction === "minimize"}
                    onChange={() => setCloseAction("minimize")}
                  />
                  <span>
                    Minimize to tray
                    <small>Closing the window hides it to the system tray; the app keeps running (and syncing) while this is on.</small>
                  </span>
                </label>
              </div>
            </div>
          </section>

          <section className="set-section">
            <h4>Cloudflare D1 sync</h4>
            <label className="check">
              <input type="checkbox" checked={syncEnabled} onChange={(e) => setSyncEnabled(e.target.checked)} />
              Sync notes with a Cloudflare D1 database
            </label>
            <div className={syncEnabled ? "" : "disabled"}>
              <p className="hint">
                Create a D1 database in the Cloudflare dashboard and an API token with the{" "}
                <b>D1 Edit</b> permission. Use the same database on every device. Tables are created
                automatically.
              </p>
              <div className="field">
                <label>Account ID</label>
                <input
                  value={accountId}
                  onChange={(e) => setAccountId(e.target.value)}
                  spellCheck={false}
                  autoComplete="off"
                />
              </div>
              <div className="field">
                <label>Database ID</label>
                <input
                  value={databaseId}
                  onChange={(e) => setDatabaseId(e.target.value)}
                  spellCheck={false}
                  autoComplete="off"
                />
              </div>
              <div className="field">
                <label>API token</label>
                <input
                  type="password"
                  value={token}
                  placeholder={settings.hasToken ? "Saved — leave blank to keep" : ""}
                  onChange={(e) => setToken(e.target.value)}
                  spellCheck={false}
                  autoComplete="off"
                />
              </div>
              <div className="field inline">
                <label className="check">
                  <input type="checkbox" checked={autoSync} onChange={(e) => setAutoSync(e.target.checked)} />
                  Sync automatically every
                </label>
                <input
                  className="num"
                  type="number"
                  min={1}
                  value={interval}
                  disabled={!autoSync}
                  onChange={(e) => setIntervalMin(Number(e.target.value))}
                />
                <span>min</span>
              </div>
              <p className="hint">
                Conflicts are resolved per item by the most recent edit, so keep your devices' clocks
                accurate. The token is stored unencrypted in this app's data folder.
              </p>
              <div className="sync-actions">
                <button
                  className="btn"
                  disabled={busy || !credentialsFilled}
                  onClick={() =>
                    guard(async () => {
                      await save();
                      await api.syncTest();
                      setNotice("Connection OK.");
                    })
                  }
                >
                  Test connection
                </button>
                <button
                  className="btn"
                  disabled={busy || syncing || !syncEnabled || !credentialsFilled}
                  onClick={() =>
                    guard(async () => {
                      await save();
                      await onSyncNow();
                      await refreshStatus();
                      setNotice("Synced.");
                    })
                  }
                >
                  <Icon name="refresh" size={13} className={syncing ? "spin" : ""} /> Sync now
                </button>
                <span className="sync-meta">
                  {notice ? (
                    <>
                      <Icon name="check" size={13} /> {notice}
                    </>
                  ) : status ? (
                    <>
                      {status.lastSyncAt ? `Last synced ${formatTime(status.lastSyncAt)}` : "Never synced"}
                      {status.pending > 0 && ` · ${status.pending} unsynced change${status.pending > 1 ? "s" : ""}`}
                    </>
                  ) : null}
                </span>
              </div>
            </div>
          </section>

          <section className="set-section">
            <h4>Updates</h4>
            <div className="update-row">
              <div className="update-current">
                <span>Version {updater.currentVersion || "…"}</span>
                <small>Updates are downloaded from GitHub Releases.</small>
              </div>
              {updater.phase === "available" || updater.phase === "ready" ? (
                <button className="btn" onClick={updater.open}>
                  <Icon name="download" size={13} />
                  {updater.phase === "ready" ? "Restart to finish" : "View update"}
                </button>
              ) : (
                <button
                  className="btn"
                  disabled={
                    updater.phase === "checking" ||
                    updater.phase === "downloading" ||
                    updater.phase === "installing"
                  }
                  onClick={() => void updater.check(true)}
                >
                  <Icon
                    name="refresh"
                    size={13}
                    className={updater.phase === "checking" ? "spin" : ""}
                  />
                  {updater.phase === "checking" ? "Checking…" : "Check for updates"}
                </button>
              )}
            </div>
            <label className="check update-auto">
              <input
                type="checkbox"
                checked={updater.autoCheck}
                onChange={(e) => updater.setAutoCheck(e.target.checked)}
              />
              Check for updates automatically when the app starts
            </label>
            {updater.message && (
              <p className="hint update-message">
                <Icon name="check" size={13} /> {updater.message}
              </p>
            )}
            {updater.error && <p className="hint update-error">{updater.error}</p>}
          </section>
        </div>

        <div className="confirm-actions">
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button
            className="primary"
            disabled={busy}
            onClick={() =>
              guard(async () => {
                await save();
                onClose();
              })
            }
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
