import { Check, ChevronDown, X } from "reicon-react";
import {
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
  type TextareaHTMLAttributes,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";

type ButtonVariant = "default" | "primary" | "highlighted" | "danger" | "error" | "ghost";

export function Button({
  className,
  variant = "default",
  size,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant; size?: "default" | "sm" }) {
  return (
    <button
      className={cn(
        variant === "ghost" ? "icon-button" : "button",
        (variant === "primary" || variant === "highlighted") && "button-highlighted",
        (variant === "danger" || variant === "error") && "button-error",
        size === "sm" && "h-7 px-2 text-xs",
        className,
      )}
      {...props}
    />
  );
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={cn("input", props.className)} />;
}

export function Textarea(props: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea {...props} className={cn("input", props.className)} />;
}

export type SelectOption<T extends string> = { value: T; label: string };

/* Custom listbox (DESIGN.md P4: no native <select>). The menu renders in a
   portal with fixed position, so a card or modal edge never clips it. */
export function Select<T extends string>({
  value,
  options,
  onChange,
  placeholder = "Choose",
  className,
  disabled,
  "aria-label": ariaLabel,
}: {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  placeholder?: string;
  className?: string;
  disabled?: boolean;
  "aria-label"?: string;
}) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [rect, setRect] = useState<DOMRect | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const listId = useId();
  const selected = options.find((option) => option.value === value);

  function show() {
    if (disabled || !trigger.current) return;
    setRect(trigger.current.getBoundingClientRect());
    setActive(Math.max(0, options.findIndex((option) => option.value === value)));
    setOpen(true);
  }

  function choose(option: SelectOption<T>) {
    onChange(option.value);
    setOpen(false);
    trigger.current?.focus();
  }

  useEffect(() => {
    if (!open) return;
    const close = (event: Event) => {
      if (event.type === "mousedown" && (trigger.current?.contains(event.target as Node) || panel.current?.contains(event.target as Node))) return;
      if (event.type === "scroll" && panel.current?.contains(event.target as Node)) return;
      setOpen(false);
    };
    document.addEventListener("mousedown", close);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", close, true);
    return () => {
      document.removeEventListener("mousedown", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("scroll", close, true);
    };
  }, [open]);

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(event.key)) {
        event.preventDefault();
        show();
      }
      return;
    }
    if (event.key === "Escape" || event.key === "Tab") setOpen(false);
    else if (event.key === "ArrowDown") setActive((index) => Math.min(options.length - 1, index + 1));
    else if (event.key === "ArrowUp") setActive((index) => Math.max(0, index - 1));
    else if ((event.key === "Enter" || event.key === " ") && options[active]) choose(options[active]);
    else return;
    event.preventDefault();
  }

  const below = rect ? window.innerHeight - rect.bottom > 280 || rect.top < 280 : true;
  return (
    <>
      <button
        ref={trigger}
        type="button"
        className={cn("input select-trigger", className)}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={ariaLabel}
        onClick={() => (open ? setOpen(false) : show())}
        onKeyDown={onKeyDown}
      >
        <span className={cn("truncate", !selected && "text-[var(--ui-fg-faint)]")}>{selected?.label ?? placeholder}</span>
        <ChevronDown className="size-4 shrink-0 text-[var(--ui-fg-faint)]" aria-hidden="true" />
      </button>
      {open && rect
        ? createPortal(
            <div
              ref={panel}
              id={listId}
              role="listbox"
              className="listbox-panel"
              style={{
                left: rect.left,
                minWidth: rect.width,
                ...(below ? { top: rect.bottom + 4 } : { bottom: window.innerHeight - rect.top + 4 }),
              }}
            >
              {options.map((option, index) => (
                <button
                  key={option.value}
                  type="button"
                  role="option"
                  aria-selected={option.value === value}
                  className={cn("listbox-option", index === active && "is-active")}
                  onMouseEnter={() => setActive(index)}
                  onClick={() => choose(option)}
                >
                  <span className="truncate">{option.label}</span>
                  {option.value === value ? <Check className="size-3.5 shrink-0 text-[var(--ui-fg)]" aria-hidden="true" /> : null}
                </button>
              ))}
            </div>,
            document.body,
          )
        : null}
    </>
  );
}

export function Field({
  label,
  hint,
  error,
  children,
}: {
  label: string;
  hint?: string;
  error?: string;
  children: ReactNode;
}) {
  return (
    <label className="block">
      <span className="field-label">
        <span>{label}</span>
        {hint ? <span className="ml-auto text-[11px] font-normal">{hint}</span> : null}
      </span>
      {children}
      {error ? <p className="field-error">{error}</p> : null}
    </label>
  );
}

/* Like Coolify, the title and summary of a top-level page show only below
   1024px; the sidebar already names the page on desktop. Detail pages keep
   their title, because it names the item that is open. */
export function PageHeader({
  title,
  description,
  actions,
  keepTitle = false,
}: {
  title: ReactNode;
  description?: string;
  actions?: ReactNode;
  keepTitle?: boolean;
}) {
  return (
    <div className={cn("page-header", !keepTitle && "is-mobile-title", !keepTitle && !actions && "is-empty-desktop")}>
      <div className="page-header-text">
        <h1>{title}</h1>
        {description ? <p>{description}</p> : null}
      </div>
      {actions ? <div className="page-header-actions flex flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  );
}

export function LayerCard({
  title,
  description,
  actions,
  flush,
  children,
  className,
}: {
  title?: string;
  description?: string;
  actions?: ReactNode;
  flush?: boolean;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("layer-card", className)}>
      {title ? (
        <header>
          <div className="min-w-0 py-0.5">
            <h2>{title}</h2>
            {description ? <p>{description}</p> : null}
          </div>
          {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
        </header>
      ) : null}
      <div className={cn("layer-card-body", flush && "is-flush", !title && "rounded-lg")}>{children}</div>
    </section>
  );
}

export function StatusBadge({
  tone = "neutral",
  children,
}: {
  tone?: "neutral" | "success" | "warning" | "error" | "info";
  children: ReactNode;
}) {
  return (
    <span className={cn("status-badge", tone)}>
      <i />
      <span className="truncate">{children}</span>
    </span>
  );
}

export function Badge({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <span className={cn("inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium", className)}>
      {children}
    </span>
  );
}

export function Dialog({
  open,
  title,
  description,
  onClose,
  children,
  footer,
  wide,
}: {
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  children: ReactNode;
  /** Actions on the right, below a divider. */
  footer?: ReactNode;
  wide?: boolean;
}) {
  if (!open) return null;
  return (
    <div className="modal-backdrop">
      <button className="absolute inset-0" aria-label="Close dialog" onClick={onClose} />
      <div className={cn("modal-panel", wide && "is-wide")} role="dialog" aria-modal="true" aria-labelledby="dialog-title">
        <div className="modal-heading">
          <div>
            <h2 id="dialog-title">{title}</h2>
            {description ? <p>{description}</p> : null}
          </div>
          <button type="button" className="icon-button" onClick={onClose} aria-label="Close">
            <X className="size-4" />
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer ? <div className="modal-footer">{footer}</div> : null}
      </div>
    </div>
  );
}

export function Empty({ title, body, action }: { title: string; body: string; action?: ReactNode }) {
  return (
    <LayerCard>
      <div className="flex min-h-40 flex-col items-center justify-center px-2 py-8 text-center">
        <h2 className="text-[14px] font-medium text-[var(--ui-fg)]">{title}</h2>
        <p className="mx-auto mt-1 max-w-[28rem] text-[13px] text-[var(--ui-fg-faint)]">{body}</p>
        {action ? <div className="mt-4">{action}</div> : null}
      </div>
    </LayerCard>
  );
}

export function ErrorText({ error }: { error: unknown }) {
  if (!error) return null;
  return <p className="field-error">{error instanceof Error ? error.message : "Something went wrong."}</p>;
}
