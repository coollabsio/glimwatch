import { Activity, Check, ChartLine, Copy, Global, Home, TickCircle } from "reicon-react";
import { useState, type ReactNode } from "react";
import { Button, Dialog, Input, LayerCard, PageHeader } from "@/components/ui";
import {
  type Appearance,
  type CustomTheme,
  accentTextOn,
  customThemePresets,
  defaultCustomColors,
  parseCustomTheme,
  readableTextOn,
  serializeCustomTheme,
  setAppearance,
  useAppearance,
} from "@/lib/theme";
import { cn } from "@/lib/utils";

/* Coolify's Appearance page: one card per preference, each option a picture
   card. Everything is stored in this browser only. */
export function AppearancePage() {
  const appearance = useAppearance();
  const [dialogOpen, setDialogOpen] = useState(false);

  return (
    <div className="flex w-full flex-col gap-6">
      <PageHeader title="Appearance" description="How Haruvo looks in this browser." />

      <LayerCard title="Color theme" description="Choose the color scheme used in this browser.">
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {(
            [
              ["light", "Light", "Bright surfaces and dark text.", "bg-white"],
              ["system", "System", "Follow your operating system.", "bg-gradient-to-r from-white via-neutral-400 to-[#050505]"],
              ["dark", "Dark", "Dark surfaces and soft contrast.", "bg-[#181818]"],
            ] as const
          ).map(([value, label, description, preview]) => (
            <Choice
              key={value}
              label={label}
              description={description}
              selected={appearance.theme === value}
              onSelect={() => setAppearance({ theme: value })}
              preview={
                <div className={cn("flex h-full items-center justify-center", preview)}>
                  <div className="h-8 w-20 rounded-md border border-black/10 bg-neutral-100/80 shadow-sm dark:border-white/10 dark:bg-black/20" />
                </div>
              }
            />
          ))}
          <Choice
            label="Custom"
            description="Pick a preset or your own colors."
            selected={appearance.theme === "custom"}
            onSelect={() => setDialogOpen(true)}
            preview={
              <div className="flex h-full items-center justify-center bg-neutral-100 dark:bg-black/20">
                <Swatch theme={appearance.customTheme ?? defaultCustomColors} className="h-12 w-24" />
              </div>
            }
          />
        </div>
      </LayerCard>

      <LayerCard title="Page width" description="Choose how content uses the available browser width.">
        <ChoiceGrid
          value={appearance.pageWidth}
          onChange={(pageWidth) => setAppearance({ pageWidth })}
          options={[
            { value: "full", label: "Full width", description: "Use all available space for page content." },
            { value: "centered", label: "Centered", description: "Keep content centered at a comfortable maximum width." },
          ]}
          preview={(value) => (
            <div className="flex h-full items-center px-4">
              <div className="flex h-11 w-full gap-1.5 rounded-md border border-neutral-300 bg-white p-1.5 dark:border-white/15 dark:bg-[#181818]">
                <div className="w-3 shrink-0 rounded-sm bg-neutral-200 dark:bg-white/10" />
                <div className={cn("h-full rounded-sm bg-neutral-200 dark:bg-white/10", value === "full" ? "w-full" : "mx-auto w-2/3")} />
              </div>
            </div>
          )}
        />
      </LayerCard>

      <LayerCard title="Navigation style" description="Choose how the top bar and sidebar sit on the page.">
        <ChoiceGrid
          value={appearance.navStyle}
          onChange={(navStyle) => setAppearance({ navStyle })}
          options={[
            { value: "default", label: "Default", description: "Top bar and sidebar run edge to edge." },
            { value: "floating", label: "Floating", description: "Framed panels float on a dotted backdrop." },
          ]}
          preview={(value) => {
            const floating = value === "floating";
            const frame = "rounded-lg border border-neutral-200 dark:border-white/[0.08]";
            return (
              <div className={cn("flex h-full", floating && "gap-1.5 p-1.5")}>
                <div
                  className={cn(
                    "flex flex-col items-center gap-1.5 bg-white px-2 py-2 dark:bg-white/[0.04]",
                    floating ? frame : "border-r border-neutral-200 dark:border-white/[0.07]",
                  )}
                >
                  {[Activity, Global].map((Icon, index) => (
                    <span
                      key={index}
                      className="flex size-6 items-center justify-center rounded-md border border-neutral-200 bg-neutral-50 text-neutral-500 dark:border-white/[0.08] dark:bg-white/[0.04] dark:text-[var(--ui-fg-dim)]"
                    >
                      <Icon className="size-3.5" />
                    </span>
                  ))}
                </div>
                <div
                  className={cn(
                    "h-5 flex-1 bg-white dark:bg-white/[0.04]",
                    floating ? frame : "border-b border-neutral-200 dark:border-white/[0.07]",
                  )}
                />
              </div>
            );
          }}
        />
      </LayerCard>

      <LayerCard title="Style" description="Choose how buttons and sidebar items look.">
        <ChoiceGrid
          value={appearance.navItems}
          onChange={(navItems) => setAppearance({ navItems })}
          options={[
            { value: "3d", label: "3D", description: "Buttons and sidebar icons sit on a raised edge that lifts on hover." },
            { value: "flat", label: "Flat", description: "No raised edges. Sidebar items are plain rows with a highlight for the current page." },
          ]}
          preview={(value) => (
            <div className="flex h-full flex-col justify-center gap-1.5 px-4">
              {[Home, ChartLine].map((Icon, index) => (
                <div
                  key={index}
                  className={cn(
                    "flex items-center gap-2 rounded-md text-neutral-500 dark:text-[var(--ui-fg-dim)]",
                    value === "flat" && "px-1.5 py-1",
                    value === "flat" && index === 0 && "bg-neutral-200/70 dark:bg-white/[0.08]",
                  )}
                >
                  <span
                    className={cn(
                      "flex items-center justify-center",
                      value === "3d" &&
                        "size-6 rounded-md border border-neutral-200 bg-white shadow-[0_2px_0_rgb(0_0_0/0.22)] dark:border-white/[0.09] dark:bg-white/[0.04] dark:shadow-[0_2px_0_rgb(255_255_255/0.08)]",
                    )}
                  >
                    <Icon className="size-3.5" />
                  </span>
                  <span className="h-1.5 w-16 rounded-full bg-neutral-200 dark:bg-white/10" />
                </div>
              ))}
            </div>
          )}
        />
      </LayerCard>

      <LayerCard title="Chart times" description="Choose which times chart tooltips show.">
        <ChoiceGrid
          value={appearance.chartTimes}
          onChange={(chartTimes) => setAppearance({ chartTimes })}
          options={[
            { value: "local", label: "Your time", description: "Show the time in the time zone of this browser." },
            { value: "utc", label: "Your time and UTC", description: "Also show the UTC time. Use it to compare with server logs." },
          ]}
          preview={(value) => (
            <div className="flex h-full items-center justify-center px-4">
              <div className="rounded-md border border-neutral-300 bg-white px-2.5 py-1.5 text-[11px] leading-4 dark:border-white/15 dark:bg-[#181818]">
                <div className="text-[var(--ui-fg)]">
                  Latency: <span className="font-semibold">42 ms</span>
                </div>
                {(value === "utc" ? ["Your time: Oct 24, 12:30:33 GMT+2", "UTC: Oct 24, 10:30:33 UTC"] : ["Oct 24, 12:30:33"]).map((line) => (
                  <div key={line} className="text-[var(--ui-fg-faint)]">
                    {line}
                  </div>
                ))}
              </div>
            </div>
          )}
        />
      </LayerCard>

      {dialogOpen ? <CustomThemeDialog appearance={appearance} onClose={() => setDialogOpen(false)} /> : null}
    </div>
  );
}

function Choice({
  label,
  description,
  selected,
  onSelect,
  preview,
}: {
  label: string;
  description: string;
  selected: boolean;
  onSelect: () => void;
  preview: ReactNode;
}) {
  return (
    <button type="button" className={cn("choice-card", selected && "is-selected")} aria-pressed={selected} onClick={onSelect}>
      <div className="choice-card-preview">{preview}</div>
      <div className="p-3">
        <div className="flex items-center justify-between gap-2">
          <span className="text-sm font-semibold text-[var(--ui-fg)]">{label}</span>
          {selected ? <TickCircle className="choice-card-check size-4" /> : null}
        </div>
        <p className="mt-1 text-xs leading-5 text-[var(--ui-fg-faint)]">{description}</p>
      </div>
    </button>
  );
}

function ChoiceGrid<T extends string>({
  value,
  onChange,
  options,
  preview,
}: {
  value: T;
  onChange: (value: T) => void;
  options: { value: T; label: string; description: string }[];
  preview: (value: T) => ReactNode;
}) {
  return (
    <div className="grid gap-3 sm:grid-cols-2">
      {options.map((option) => (
        <Choice
          key={option.value}
          label={option.label}
          description={option.description}
          selected={value === option.value}
          onSelect={() => onChange(option.value)}
          preview={preview(option.value)}
        />
      ))}
    </div>
  );
}

/** Small picture of a custom theme: sidebar with the selected row, page with a card. */
function Swatch({ theme, className }: { theme: CustomTheme; className?: string }) {
  const onSidebar = readableTextOn(theme.sidebar);
  const onPage = readableTextOn(theme.background);
  return (
    <div className={cn("flex overflow-hidden rounded-md border border-black/10 dark:border-white/10", className)} aria-hidden="true">
      <div className="flex w-[38%] flex-col gap-1 p-1.5" style={{ background: theme.sidebar }}>
        <span className="h-1 w-3/4 rounded-full opacity-50" style={{ background: onSidebar }} />
        <span className="h-2.5 w-full rounded-sm" style={{ background: theme.selected }} />
        <span className="flex items-center gap-1">
          <span className="h-1 w-1/2 rounded-full opacity-50" style={{ background: onSidebar }} />
          <span className="size-1.5 rounded-[2px]" style={{ background: theme.accent }} />
        </span>
      </div>
      <div className="flex flex-1 flex-col gap-1 p-1.5" style={{ background: theme.background }}>
        <span className="h-1 w-2/3 rounded-full opacity-60" style={{ background: onPage }} />
        <span className="h-1 w-1/2 rounded-full opacity-30" style={{ background: onPage }} />
        <span className="mt-auto h-2.5 w-full rounded-sm border" style={{ borderColor: `color-mix(in oklab, ${onPage} 25%, transparent)` }} />
      </div>
    </div>
  );
}

const colorFields: { key: keyof CustomTheme; label: string; description: string }[] = [
  { key: "sidebar", label: "Sidebar", description: "The navigation on the left" },
  { key: "selected", label: "Selected", description: "The current page in the sidebar" },
  { key: "accent", label: "Accent", description: "Buttons, links, and highlights" },
  { key: "background", label: "Background", description: "Pages, cards, and panels" },
];

/* Presets, a preview, four colors, and a share code. Save applies the draft.
   A null draft is the built-in Dark theme. */
function CustomThemeDialog({ appearance, onClose }: { appearance: Appearance; onClose: () => void }) {
  const [draft, setDraft] = useState<CustomTheme | null>(appearance.theme === "custom" ? appearance.customTheme : null);
  const [code, setCode] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const colors = draft ?? defaultCustomColors;
  const shareCode = serializeCustomTheme(colors);
  const codeInvalid = code !== null && code.trim() !== "" && !parseCustomTheme(code);
  const onSidebar = readableTextOn(colors.sidebar);
  const onPage = readableTextOn(colors.background);
  const card = {
    background: `color-mix(in oklab, ${onPage} 4%, ${colors.background})`,
    borderColor: `color-mix(in oklab, ${onPage} 14%, ${colors.background})`,
  };

  function setColor(key: keyof CustomTheme, value: string) {
    const theme = parseCustomTheme(serializeCustomTheme({ ...colors, [key]: value }));
    if (theme) setDraft(theme);
  }

  function save() {
    if (draft) setAppearance({ theme: "custom", customTheme: draft });
    else setAppearance({ theme: "dark" });
    onClose();
  }

  async function copy() {
    await navigator.clipboard?.writeText(shareCode);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <Dialog
      open
      wide
      title="Custom theme"
      description="Colors of Haruvo in this browser. Text stays readable on every color."
      onClose={onClose}
      footer={
        <>
          <Button type="button" onClick={onClose}>
            Cancel
          </Button>
          <Button type="button" variant="highlighted" onClick={save}>
            Save theme
          </Button>
        </>
      }
    >
      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)]">
        <div>
          <div className="mb-2 text-xs font-medium text-[var(--ui-fg-faint)]">Presets</div>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
            {customThemePresets.map((preset) => {
              const pressed =
                preset.theme === null || draft === null
                  ? preset.theme === draft
                  : serializeCustomTheme(preset.theme) === serializeCustomTheme(draft);
              return (
                <button
                  key={preset.name}
                  type="button"
                  aria-pressed={pressed}
                  className="preset-button"
                  onClick={() => {
                    setDraft(preset.theme && { ...preset.theme });
                    setCode(null);
                  }}
                >
                  <Swatch theme={preset.theme ?? preset.swatch!} className={cn("h-14 w-full", pressed && "is-pressed")} />
                  <span className="truncate text-xs text-[var(--ui-fg-dim)]">{preset.name}</span>
                </button>
              );
            })}
          </div>
        </div>

        <div className="flex min-w-0 flex-col gap-5">
          <div className="flex h-48 overflow-hidden rounded-lg border border-[var(--ui-divider)] text-xs" aria-hidden="true">
            <div className="flex w-2/5 flex-col gap-0.5 p-2.5" style={{ background: colors.sidebar, color: onSidebar }}>
              <div className="mb-2 px-2 text-sm font-semibold">Haruvo</div>
              <div className="flex items-center gap-2 rounded-md px-2 py-1.5 opacity-75">
                <Home className="size-3.5" /> Dashboard
              </div>
              <div
                className="flex items-center gap-2 rounded-md px-2 py-1.5 font-medium"
                style={{ background: colors.selected, color: readableTextOn(colors.selected) }}
              >
                <Activity className="size-3.5" /> Monitors
              </div>
              <div className="flex items-center gap-2 rounded-md px-2 py-1.5 opacity-75">
                <Global className="size-3.5" /> Status pages
                <span
                  className="ml-auto rounded-full px-1.5 text-[10px] font-semibold"
                  style={{ background: colors.accent, color: accentTextOn(colors.accent) }}
                >
                  3
                </span>
              </div>
            </div>
            <div className="flex min-w-0 flex-1 flex-col gap-2 p-3" style={{ background: colors.background, color: onPage }}>
              <div className="flex items-center justify-between gap-2">
                <span className="text-sm font-semibold">Monitors</span>
                <span
                  className="rounded-md px-2 py-1 text-[11px] font-medium"
                  style={{ background: colors.accent, color: accentTextOn(colors.accent) }}
                >
                  + New
                </span>
              </div>
              <div className="rounded-md border p-2.5" style={card}>
                <div className="font-medium">API</div>
                <div className="mt-0.5 opacity-60">Operational</div>
              </div>
              <div className="rounded-md border p-2.5" style={card}>
                <div className="font-medium">Website</div>
                <div className="mt-0.5 opacity-60">Operational</div>
              </div>
            </div>
          </div>

          <div>
            <div className="mb-2 text-xs font-medium text-[var(--ui-fg-faint)]">Colors</div>
            <div className="flex flex-col gap-2.5">
              {colorFields.map((field) => (
                <div key={field.key} className="flex items-center gap-3">
                  <label className="relative size-9 shrink-0 cursor-pointer overflow-hidden rounded-md border border-black/10 shadow-sm dark:border-white/15">
                    <span className="absolute inset-0" style={{ background: colors[field.key] }} />
                    <input
                      type="color"
                      autoComplete="off"
                      value={colors[field.key]}
                      onChange={(event) => setColor(field.key, event.target.value)}
                      aria-label={`${field.label} color`}
                      className="absolute inset-0 h-full w-full cursor-pointer opacity-0"
                    />
                  </label>
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium text-[var(--ui-fg)]">{field.label}</div>
                    <div className="truncate text-xs text-[var(--ui-fg-faint)]">{field.description}</div>
                  </div>
                  <div className="w-28 shrink-0">
                    <Input
                      key={colors[field.key]}
                      defaultValue={colors[field.key]}
                      maxLength={7}
                      spellCheck={false}
                      autoComplete="off"
                      aria-label={`${field.label} hex color`}
                      className="font-mono text-sm"
                      onBlur={(event) => setColor(field.key, event.target.value)}
                      onKeyDown={(event) => event.key === "Enter" && setColor(field.key, event.currentTarget.value)}
                    />
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div>
            <label htmlFor="custom-theme-share-code" className="mb-2 block text-xs font-medium text-[var(--ui-fg-faint)]">
              Share code
            </label>
            <div className="flex gap-2">
              <Input
                id="custom-theme-share-code"
                value={code ?? shareCode}
                spellCheck={false}
                autoComplete="off"
                aria-invalid={codeInvalid}
                className={cn("min-w-0 flex-1 font-mono text-sm", codeInvalid && "border-[var(--ui-error)]")}
                onChange={(event) => {
                  setCode(event.target.value);
                  const theme = parseCustomTheme(event.target.value);
                  if (theme) setDraft(theme);
                }}
                onBlur={() => setCode(null)}
              />
              <Button type="button" onClick={() => void copy()} aria-label="Copy share code">
                {copied ? <Check className="size-4 text-[var(--ui-success)]" /> : <Copy className="size-4" />}
              </Button>
            </div>
            <p className={cn("mt-1.5 text-xs", codeInvalid ? "text-[var(--ui-error)]" : "text-[var(--ui-fg-faint)]")}>
              {codeInvalid ? "Enter four hex colors, separated by commas." : "Paste a code that someone shared, or copy yours."}
            </p>
          </div>
        </div>
      </div>
    </Dialog>
  );
}
