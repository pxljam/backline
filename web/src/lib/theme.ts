/**
 * Dark or light, and who decides.
 *
 * The application is dark by default — it is a backstage console, and the one
 * sheet of paper on screen should be the Studio artboard. But someone working
 * in daylight, or anyone who reads pale-on-black badly, picks the other side in
 * Réglages and that choice wins over the system preference.
 *
 * Applied before React mounts (see `main.tsx`), so no frame is ever painted in
 * the wrong theme.
 */

export type Theme = "dark" | "light" | "system";

const KEY = "backline.theme";

export function storedTheme(): Theme {
  try {
    const value = localStorage.getItem(KEY);
    if (value === "dark" || value === "light" || value === "system") return value;
  } catch {
    // Private browsing, blocked storage: the default is a fine answer.
  }
  return "dark";
}

export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);

  // Native widgets — the collective switcher, the date inputs — read this and
  // nothing else. Left wrong, they open as white popups on a black page.
  const meta = document.querySelector('meta[name="color-scheme"]');
  if (meta) {
    meta.setAttribute(
      "content",
      theme === "system" ? "dark light" : theme === "dark" ? "dark" : "light",
    );
  }
}

export function setTheme(theme: Theme): void {
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // The theme still applies for this session; only the memory is lost.
  }
  applyTheme(theme);
}

export function applyStoredTheme(): void {
  applyTheme(storedTheme());
}
