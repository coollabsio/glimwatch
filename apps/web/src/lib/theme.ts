import { useSyncExternalStore } from "react";

/* Appearance preferences of this browser (Coolify's Appearance page). They
   live in one localStorage entry. The inline script in index.html reads the
   same entry and applies it before the first paint, so it also stores the
   computed variables of a custom theme. */

export type ThemeMode = "light" | "system" | "dark" | "custom";
export type CustomTheme = { sidebar: string; selected: string; accent: string; background: string };
export type Appearance = {
  theme: ThemeMode;
  customTheme: CustomTheme | null;
  pageWidth: "centered" | "full";
  navStyle: "default" | "floating";
  navItems: "3d" | "flat";
  chartTimes: "local" | "utc";
};

const KEY = "haruvo-appearance";
const EVENT = "haruvo-appearance";
const customKeys = ["sidebar", "selected", "accent", "background"] as const;

export const customThemePresets: { name: string; theme: CustomTheme | null; swatch?: CustomTheme }[] = [
  // The built-in Dark theme, not a custom theme. Its colors start the fields when you edit it.
  { name: "Haruvo", theme: null, swatch: { sidebar: "#111a2e", selected: "#1c1c1c", accent: "#2563eb", background: "#0a0a0a" } },
  { name: "Midnight", theme: { sidebar: "#0f172a", selected: "#2e4a7d", accent: "#f2458f", background: "#1a2338" } },
  { name: "Forest", theme: { sidebar: "#12342b", selected: "#2c6e57", accent: "#f4b860", background: "#1b2a25" } },
  { name: "Ember", theme: { sidebar: "#3b1410", selected: "#8f2d1f", accent: "#ffb86b", background: "#241816" } },
  { name: "Slate", theme: { sidebar: "#2b3038", selected: "#4a5361", accent: "#6cc4f5", background: "#353b45" } },
  { name: "Nord", theme: { sidebar: "#2e3440", selected: "#4c566a", accent: "#88c0d0", background: "#3b4252" } },
  { name: "Dracula", theme: { sidebar: "#21222c", selected: "#44475a", accent: "#bd93f9", background: "#282a36" } },
  { name: "GitHub Dark", theme: { sidebar: "#010409", selected: "#21262d", accent: "#2f81f7", background: "#0d1117" } },
  { name: "Aubergine", theme: { sidebar: "#3f0e40", selected: "#1164a3", accent: "#1164a3", background: "#ffffff" } },
  { name: "GitHub Light", theme: { sidebar: "#f6f8fa", selected: "#dde4ec", accent: "#0969da", background: "#ffffff" } },
  { name: "Paper", theme: { sidebar: "#ebe6da", selected: "#d9d1bf", accent: "#c2410c", background: "#f7f4ec" } },
  { name: "Blossom", theme: { sidebar: "#fde8f1", selected: "#f9c4dc", accent: "#d61f69", background: "#fff7fa" } },
];

export const defaultCustomColors = customThemePresets[0].swatch!;

function hex(color: unknown) {
  const value = String(color ?? "").trim().replace(/^#/, "").toLowerCase();
  if (/^[a-f\d]{6}$/.test(value)) return `#${value}`;
  if (/^[a-f\d]{3}$/.test(value)) return `#${value.replace(/./g, "$&$&")}`;
  return null;
}

const linear = (channel: number) => (channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4);
const channels = (color: string) => color.match(/[a-f\d]{2}/gi)!.map((part) => parseInt(part, 16) / 255);
const luminance = (color: string) =>
  channels(color).map(linear).reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index], 0);

/** True when light text has more contrast on this color than dark text (WCAG ratios). */
export function isDarkColor(color: string) {
  const l = luminance(color);
  return 1.05 / (l + 0.05) >= (l + 0.05) / (luminance("#18181b") + 0.05);
}

export const readableTextOn = (color: string) => (isDarkColor(color) ? "#f4f4f5" : "#18181b");

/** Text on an accent fill: black or white, whichever reads on the fill. */
export function accentTextOn(color: string) {
  const l = channels(color)
    .map((channel) => linear(channel * 0.85 + 0.15))
    .reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index], 0);
  return l > 0.179 ? "#000000" : "#ffffff";
}

export const serializeCustomTheme = (theme: CustomTheme) => customKeys.map((key) => theme[key]).join(",");

/** A theme from a share code ("#sidebar,#selected,#accent,#background"), or null. */
export function parseCustomTheme(code: string): CustomTheme | null {
  const colors = code.split(/[\s,]+/).filter(Boolean).map(hex);
  if (colors.length !== 4 || colors.includes(null)) return null;
  return Object.fromEntries(customKeys.map((key, index) => [key, colors[index]])) as CustomTheme;
}

function read(): Appearance {
  let saved: Partial<Appearance> = {};
  try {
    const raw = localStorage.getItem(KEY) ?? localStorage.getItem("statixo-appearance");
    saved = JSON.parse(raw ?? "{}") ?? {};
  } catch {
    // A broken entry falls back to the defaults.
  }
  const legacy = localStorage.getItem("haruvo-theme");
  const customTheme = saved.customTheme ? parseCustomTheme(serializeCustomTheme(saved.customTheme)) : null;
  const theme = (["light", "system", "dark", "custom"] as const).find((mode) => mode === (saved.theme ?? legacy)) ?? "dark";
  return {
    theme: theme === "custom" && !customTheme ? "dark" : theme,
    customTheme,
    pageWidth: saved.pageWidth === "full" ? "full" : "centered",
    navStyle: saved.navStyle === "floating" ? "floating" : "default",
    navItems: saved.navItems === "flat" ? "flat" : "3d",
    chartTimes: saved.chartTimes === "utc" ? "utc" : "local",
  };
}

/** CSS variables that index.css derives the custom theme from. */
function customVars(theme: CustomTheme) {
  return {
    "--theme-bg": theme.background,
    "--theme-text": readableTextOn(theme.background),
    "--theme-light": isDarkColor(theme.background) ? "0" : "1",
    "--theme-sidebar": theme.sidebar,
    "--theme-sidebar-text": readableTextOn(theme.sidebar),
    "--theme-sidebar-light": isDarkColor(theme.sidebar) ? "0" : "1",
    "--theme-selected": theme.selected,
    "--theme-selected-text": readableTextOn(theme.selected),
    "--theme-accent": theme.accent,
    "--theme-accent-fg": accentTextOn(theme.accent),
  };
}

let current = read();
const media = window.matchMedia("(prefers-color-scheme: dark)");

function apply(appearance: Appearance) {
  const root = document.documentElement;
  const custom = appearance.theme === "custom" && appearance.customTheme ? customVars(appearance.customTheme) : null;
  for (const name of Object.keys(customVars(defaultCustomColors))) root.style.removeProperty(name);
  if (custom) for (const [name, value] of Object.entries(custom)) root.style.setProperty(name, value);
  const dark = custom ? custom["--theme-light"] === "0" : appearance.theme === "dark" || (appearance.theme === "system" && media.matches);
  root.classList.toggle("dark", dark);
  root.dataset.theme = custom ? "custom" : dark ? "dark" : "light";
  root.dataset.navStyle = appearance.navStyle;
  root.dataset.navItems = appearance.navItems;
  root.dataset.pageWidth = appearance.pageWidth;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", custom ? custom["--theme-bg"] : dark ? "#0a0a0a" : "#ffffff");
  return custom;
}

export function setAppearance(patch: Partial<Appearance>) {
  current = { ...current, ...patch };
  const vars = apply(current);
  localStorage.setItem(KEY, JSON.stringify({ ...current, vars }));
  localStorage.removeItem("haruvo-theme");
  localStorage.removeItem("statixo-appearance");
  window.dispatchEvent(new Event(EVENT));
}

apply(current);
media.addEventListener("change", () => current.theme === "system" && apply(current));

function subscribe(onChange: () => void) {
  window.addEventListener(EVENT, onChange);
  return () => window.removeEventListener(EVENT, onChange);
}

export function useAppearance() {
  return useSyncExternalStore(subscribe, () => current);
}

/** Time lines for a chart tooltip: your time, plus UTC when Chart times asks for it. */
export function chartTime(iso: string) {
  const format = (timeZone?: string) =>
    new Date(iso).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hourCycle: "h23",
      ...(timeZone ? { timeZone, timeZoneName: "short" } : {}),
    });
  if (current.chartTimes !== "utc") return format();
  return `Your time: ${format(Intl.DateTimeFormat().resolvedOptions().timeZone)}\nUTC: ${format("UTC")}`;
}
