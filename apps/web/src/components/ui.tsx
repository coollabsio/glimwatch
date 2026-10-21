import { X } from "lucide-react";
import {
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from "react";
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

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={cn("select", props.className)} />;
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

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: ReactNode;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <div className="page-header">
      <div>
        <h1>{title}</h1>
        {description ? <p>{description}</p> : null}
      </div>
      {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
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
}: {
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  children: ReactNode;
}) {
  if (!open) return null;
  return (
    <div className="modal-backdrop">
      <button className="absolute inset-0" aria-label="Close dialog" onClick={onClose} />
      <div className="modal-panel" role="dialog" aria-modal="true" aria-labelledby="dialog-title">
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
      </div>
    </div>
  );
}

export function Empty({ title, body, action }: { title: string; body: string; action?: ReactNode }) {
  return (
    <LayerCard>
      <div className="px-2 py-8 text-center">
        <h2 className="text-sm font-medium text-[var(--color-fg)]">{title}</h2>
        <p className="mx-auto mt-1 max-w-md text-[13px] text-[var(--coollabs-subtle)]">{body}</p>
        {action ? <div className="mt-4">{action}</div> : null}
      </div>
    </LayerCard>
  );
}

export function ErrorText({ error }: { error: unknown }) {
  if (!error) return null;
  return <p className="field-error">{error instanceof Error ? error.message : "Something went wrong."}</p>;
}
