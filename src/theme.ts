export type ThemePref = "system" | "light" | "dark";

const KEY = "theme";
const darkQuery = () => window.matchMedia("(prefers-color-scheme: dark)");

export function getThemePref(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    /* storage unavailable: fall through to default */
  }
  return "system";
}

/** Resolve the preference to a concrete theme and put it on <html>. */
export function applyTheme(pref: ThemePref) {
  const resolved = pref === "system" ? (darkQuery().matches ? "dark" : "light") : pref;
  const root = document.documentElement;
  root.dataset.theme = resolved;
  root.style.colorScheme = resolved; // native controls (select, scrollbars, color picker)
}

export function setThemePref(pref: ThemePref) {
  try {
    localStorage.setItem(KEY, pref);
  } catch {
    /* storage unavailable: applies for this session only */
  }
  applyTheme(pref);
}

/** Apply the saved theme and follow OS changes while the preference is "system". */
export function initTheme() {
  applyTheme(getThemePref());
  darkQuery().addEventListener("change", () => {
    if (getThemePref() === "system") applyTheme("system");
  });
}

export const nextThemePref = (p: ThemePref): ThemePref =>
  p === "system" ? "light" : p === "light" ? "dark" : "system";
