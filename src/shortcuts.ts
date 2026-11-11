export const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
export const modLabel = isMac ? "⌘" : "Ctrl";

/** Cross-component commands, sent as window events so components stay decoupled. */
export const EVT_SAVE_NOW = "essays:save-now";
export const EVT_VIEW_MODE = "essays:view-mode";

export const SHORTCUTS: { keys: string; action: string }[] = [
  { keys: `${modLabel}+N`, action: "New note in the selected notebook" },
  { keys: `${modLabel}+F`, action: "Search notes" },
  { keys: `${modLabel}+S`, action: "Save now" },
  { keys: `${modLabel}+1 / 2 / 3`, action: "Editor / split / preview" },
  { keys: `${modLabel}+,`, action: "Open settings" },
  { keys: `${modLabel}+/`, action: "Show this help" },
];

export interface ShortcutHandlers {
  newNote: () => void;
  focusSearch: () => void;
  toggleHelp: () => void;
  openSettings: () => void;
}

/** Returns true if the event was a known shortcut (and was handled). */
export function handleShortcut(e: KeyboardEvent, h: ShortcutHandlers): boolean {
  const mod = isMac ? e.metaKey : e.ctrlKey;
  // Don't hijack keys while an IME composition is in progress, or with extra modifiers.
  if (!mod || e.altKey || e.shiftKey || e.isComposing) return false;
  switch (e.key.toLowerCase()) {
    case "n":
      h.newNote();
      break;
    case "f":
      h.focusSearch();
      break;
    case "s":
      window.dispatchEvent(new Event(EVT_SAVE_NOW));
      break;
    case "1":
    case "2":
    case "3":
      window.dispatchEvent(
        new CustomEvent(EVT_VIEW_MODE, { detail: (["edit", "split", "preview"] as const)[+e.key - 1] }),
      );
      break;
    case ",":
      h.openSettings();
      break;
    case "/":
      h.toggleHelp();
      break;
    default:
      return false;
  }
  e.preventDefault();
  return true;
}
