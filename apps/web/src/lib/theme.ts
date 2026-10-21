import { useEffect, useState } from "react";

export type ThemeMode = "light" | "dark";

const EVENT = "status-theme";

export function currentTheme(): ThemeMode {
  return document.documentElement.classList.contains("dark") ? "dark" : "light";
}

export function applyTheme(mode: ThemeMode) {
  document.documentElement.classList.toggle("dark", mode === "dark");
  localStorage.setItem("haruvo-theme", mode);
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", mode === "dark" ? "#101010" : "#ffffff");
  window.dispatchEvent(new Event(EVENT));
}

export function useTheme() {
  const [mode, setMode] = useState<ThemeMode>(() => currentTheme());
  useEffect(() => {
    const sync = () => setMode(currentTheme());
    window.addEventListener(EVENT, sync);
    return () => window.removeEventListener(EVENT, sync);
  }, []);
  return [mode, applyTheme] as const;
}
