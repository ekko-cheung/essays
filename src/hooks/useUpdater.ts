import { useCallback, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check as checkUpdate, type Update } from "@tauri-apps/plugin-updater";

const AUTO_CHECK_KEY = "autoCheckUpdates";
const DISMISSED_KEY = "dismissedUpdateVersion";
const AUTO_CHECK_DELAY_MS = 3_000;
const CHECK_TIMEOUT_MS = 20_000;

export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "installing"
  | "ready"
  | "error";

export interface Updater {
  /** The running app version, e.g. "0.1.0". */
  currentVersion: string;
  phase: UpdatePhase;
  update: Update | null;
  /** Set for manual checks; holds "You're up to date." or an error. */
  message: string | null;
  error: string | null;
  downloaded: number;
  total: number;
  autoCheck: boolean;
  dialogOpen: boolean;
  setAutoCheck: (value: boolean) => void;
  check: (manual?: boolean) => Promise<void>;
  install: () => Promise<void>;
  restart: () => Promise<void>;
  dismiss: () => void;
  open: () => void;
}

function readAutoCheck(): boolean {
  try {
    return localStorage.getItem(AUTO_CHECK_KEY) !== "0";
  } catch {
    return true;
  }
}

function readDismissed(): string | null {
  try {
    return localStorage.getItem(DISMISSED_KEY);
  } catch {
    return null;
  }
}

/**
 * Owns the app's single update check: startup checks, the update dialog state,
 * download progress and restart. `ready` is false until the main UI can render,
 * so the dialog never appears behind the splash screen.
 */
export function useUpdater(ready: boolean): Updater {
  const [currentVersion, setCurrentVersion] = useState("");
  const [phase, setPhase] = useState<UpdatePhase>("idle");
  const [update, setUpdate] = useState<Update | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [downloaded, setDownloaded] = useState(0);
  const [total, setTotal] = useState(0);
  const [autoCheck, setAutoCheckState] = useState(readAutoCheck);
  const [dialogOpen, setDialogOpen] = useState(false);

  // The Update object is a Rust-side resource, so keep exactly one and close it
  // when it is replaced or dismissed without installing.
  const updateRef = useRef<Update | null>(null);
  const workingRef = useRef(false);
  const phaseRef = useRef<UpdatePhase>("idle");
  const autoCheckedRef = useRef(false);

  const setPhaseBoth = useCallback((next: UpdatePhase) => {
    phaseRef.current = next;
    setPhase(next);
  }, []);

  const releaseUpdate = useCallback(() => {
    const current = updateRef.current;
    updateRef.current = null;
    setUpdate(null);
    if (current) void current.close().catch(() => {});
  }, []);

  useEffect(() => {
    getVersion()
      .then(setCurrentVersion)
      .catch(() => {});
  }, []);

  // Close the updater resource if the app is torn down mid-session.
  useEffect(
    () => () => {
      const current = updateRef.current;
      updateRef.current = null;
      if (current) void current.close().catch(() => {});
    },
    [],
  );

  const check = useCallback(
    async (manual = false) => {
      if (workingRef.current) return;
      if (phaseRef.current === "downloading" || phaseRef.current === "installing") return;
      workingRef.current = true;
      setPhaseBoth("checking");
      setError(null);
      setMessage(null);
      try {
        const next = await checkUpdate({ timeout: CHECK_TIMEOUT_MS });
        if (next) {
          if (!manual && readDismissed() === next.version) {
            await next.close().catch(() => {});
            setPhaseBoth("idle");
            return;
          }
          // Replace any previously found update so its resource doesn't leak.
          if (updateRef.current) await updateRef.current.close().catch(() => {});
          updateRef.current = next;
          setUpdate(next);
          setPhaseBoth("available");
          setDialogOpen(true);
        } else {
          setPhaseBoth("idle");
          if (manual) setMessage("You're up to date.");
        }
      } catch (e) {
        if (manual) {
          setPhaseBoth("error");
          setError(String(e));
        } else {
          // Startup checks shouldn't nag: offline or rate-limited is normal.
          setPhaseBoth("idle");
          console.warn("Update check failed:", e);
        }
      } finally {
        workingRef.current = false;
      }
    },
    [setPhaseBoth],
  );

  // One silent check per launch, once the UI is ready. Disabled in `vite dev`,
  // where a release build would otherwise offer to replace the dev app.
  useEffect(() => {
    if (!ready || autoCheckedRef.current || !autoCheck || import.meta.env.DEV) return;
    autoCheckedRef.current = true;
    const timer = window.setTimeout(() => void check(false), AUTO_CHECK_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [ready, autoCheck, check]);

  const install = useCallback(async () => {
    const current = updateRef.current;
    if (!current || workingRef.current) return;
    workingRef.current = true;
    setPhaseBoth("downloading");
    setError(null);
    setDownloaded(0);
    setTotal(0);
    try {
      await current.downloadAndInstall((event) => {
        if (event.event === "Started") {
          setTotal(event.data.contentLength ?? 0);
        } else if (event.event === "Progress") {
          setDownloaded((bytes) => bytes + event.data.chunkLength);
        } else {
          setPhaseBoth("installing");
        }
      });
      // Windows exits into the installer, so this is mostly macOS.
      setPhaseBoth("ready");
    } catch (e) {
      setPhaseBoth("error");
      setError(String(e));
    } finally {
      workingRef.current = false;
    }
  }, [setPhaseBoth]);

  const restart = useCallback(async () => {
    try {
      await relaunch();
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const dismiss = useCallback(() => {
    setDialogOpen(false);
    if (phaseRef.current === "available") {
      const current = updateRef.current;
      if (current) {
        try {
          localStorage.setItem(DISMISSED_KEY, current.version);
        } catch {
          /* storage unavailable: it'll offer again next launch */
        }
      }
      releaseUpdate();
      setPhaseBoth("idle");
    }
    // When installed ("ready"), keep the state so Settings can offer a restart.
  }, [releaseUpdate, setPhaseBoth]);

  const setAutoCheck = useCallback((value: boolean) => {
    setAutoCheckState(value);
    try {
      localStorage.setItem(AUTO_CHECK_KEY, value ? "1" : "0");
    } catch {
      /* just don't remember */
    }
  }, []);

  const open = useCallback(() => {
    if (updateRef.current) setDialogOpen(true);
  }, []);

  return {
    currentVersion,
    phase,
    update,
    message,
    error,
    downloaded,
    total,
    autoCheck,
    dialogOpen,
    setAutoCheck,
    check,
    install,
    restart,
    dismiss,
    open,
  };
}
