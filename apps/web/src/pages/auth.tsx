import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, type ReactNode, useState } from "react";
import { Link, useNavigate } from "react-router";
import { Logo } from "@/app/Shell";
import { Button, ErrorText, Field, Input } from "@/components/ui";
import { ApiError, api, type Meta, type User } from "@/lib/api";

export function LoginPage() {
  const meta = useQuery({ queryKey: ["meta"], queryFn: () => api<Meta>("/api/meta") });
  const footer = meta.data?.signup_open ? (
    <>
      Need an account? <Link className="auth-text-link" to="/register">Create one</Link>
    </>
  ) : meta.data && !meta.data.setup_complete ? (
    <>Open the one-time setup URL printed when the server started.</>
  ) : null;
  return (
    <AuthShell
      title="Sign in"
      subtitle="Status pages and uptime for the coolLabs stack."
      footer={footer}
    >
      <AuthForm mode="login" />
    </AuthShell>
  );
}

export function RegisterPage() {
  const meta = useQuery({ queryKey: ["meta"], queryFn: () => api<Meta>("/api/meta") });
  if (meta.isLoading) {
    return (
      <AuthShell title="Create an account" subtitle="Checking registration." footer={null}>
        <p className="text-sm text-[var(--ui-fg-faint)]">Opening…</p>
      </AuthShell>
    );
  }
  if (!meta.data?.signup_open) {
    return (
      <AuthShell
        title="Registration is closed"
        subtitle="This Haruvo does not accept public sign-up."
        footer={
          <>
            Already have an account? <Link className="auth-text-link" to="/login">Sign in</Link>
          </>
        }
      >
        <p className="text-sm text-[var(--ui-fg-faint)]">
          The first account is created from the one-time setup link printed by the server. Public sign-up stays closed.
        </p>
      </AuthShell>
    );
  }
  return (
    <AuthShell
      title="Create an account"
      subtitle="Public registration is open on this install."
      footer={
        <>
          Already registered? <Link className="auth-text-link" to="/login">Sign in</Link>
        </>
      }
    >
      <AuthForm mode="register" />
    </AuthShell>
  );
}

export function SetupPage() {
  const token = useConsumedToken();
  if (!token) {
    return (
      <AuthShell
        title="Set up Haruvo"
        subtitle="The first account becomes the owner of this install."
        footer={
          <>
            Already set up? <Link className="auth-text-link" to="/login">Sign in</Link>
          </>
        }
      >
        <p className="text-sm text-[var(--ui-fg-faint)]">
          Open the one-time setup link printed by the server. The account you create there becomes the owner.
        </p>
      </AuthShell>
    );
  }
  return (
    <AuthShell
      title="Create the owner account"
      subtitle="This account manages the install. Setup can only be completed once."
      footer={
        <>
          Already set up? <Link className="auth-text-link" to="/login">Sign in</Link>
        </>
      }
    >
      <SetupForm token={token} />
    </AuthShell>
  );
}

function useConsumedToken() {
  const [token] = useState(() => {
    const url = new URL(window.location.href);
    const value = url.searchParams.get("token");
    if (!value) return null;
    url.searchParams.delete("token");
    window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
    return value;
  });
  return token;
}

function AuthShell({
  title,
  subtitle,
  footer,
  children,
}: {
  title: string;
  subtitle: string;
  footer: ReactNode;
  children: ReactNode;
}) {
  const meta = useQuery({ queryKey: ["meta"], queryFn: () => api<Meta>("/api/meta") });
  return (
    <section className="auth-shell">
      <div className="auth-shell-content">
        <div className="w-full max-w-[27rem]">
          <div className="mb-5 text-center">
            <Logo className="mx-auto mb-3 size-10" />
            <div className="text-[15px] font-semibold tracking-tight text-[var(--ui-fg)]">Haruvo</div>
            <div className="text-[11px] text-[var(--ui-fg-faint)]">coolLabs</div>
          </div>
          <div className="auth-card">
            <div className="auth-card-heading">
              <h1>{title}</h1>
              <p>{subtitle}</p>
            </div>
            <div className="auth-card-body">{children}</div>
            {footer ? <footer className="auth-card-footer">{footer}</footer> : null}
          </div>
          {meta.data?.demo ? (
            <p className="mt-4 text-center text-xs text-[var(--ui-fg-faint)]">
              Seeded demo: {meta.data.demo.email} / {meta.data.demo.password}
            </p>
          ) : null}
        </div>
      </div>
    </section>
  );
}

function SetupForm({ token }: { token: string }) {
  const navigate = useNavigate();
  const client = useQueryClient();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [fields, setFields] = useState<Record<string, string>>({});
  const mutation = useMutation({
    mutationFn: () =>
      api<User>("/api/setup/complete", {
        method: "POST",
        body: JSON.stringify({
          token,
          email,
          password,
          display_name: displayName,
        }),
      }),
    onSuccess: async (user) => {
      client.setQueryData(["me"], user);
      navigate("/");
    },
    onError: (error) => {
      setFields(error instanceof ApiError ? (error.fields ?? {}) : {});
    },
  });

  function submit(event: FormEvent) {
    event.preventDefault();
    setFields({});
    mutation.mutate();
  }

  return (
    <form className="space-y-4" onSubmit={submit}>
      <p className="text-[13px] text-[var(--ui-fg-faint)]">
        This account becomes the owner. Keep the password. The setup link works once.
      </p>
      <Field label="Name" error={fields.display_name}>
        <Input value={displayName} onChange={(event) => setDisplayName(event.target.value)} placeholder="Andras Bacsai" />
      </Field>
      <Field label="Email" error={fields.email}>
        <Input type="email" required value={email} onChange={(event) => setEmail(event.target.value)} placeholder="you@coollabs.io" />
      </Field>
      <Field label="Password" hint="12+ characters" error={fields.password}>
        <Input type="password" required value={password} onChange={(event) => setPassword(event.target.value)} />
      </Field>
      <ErrorText error={mutation.error} />
      <Button className="w-full" variant="primary" type="submit" disabled={mutation.isPending}>
        {mutation.isPending ? "Working…" : "Create owner account"}
      </Button>
    </form>
  );
}

function AuthForm({ mode }: { mode: "login" | "register" }) {
  const navigate = useNavigate();
  const client = useQueryClient();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [fields, setFields] = useState<Record<string, string>>({});
  const mutation = useMutation({
    mutationFn: () =>
      api<User>(mode === "login" ? "/api/auth/login" : "/api/auth/register", {
        method: "POST",
        body: JSON.stringify({
          email,
          password,
          display_name: mode === "register" ? displayName : undefined,
        }),
      }),
    onSuccess: async (user) => {
      client.setQueryData(["me"], user);
      navigate("/");
    },
    onError: (error) => {
      setFields(error instanceof ApiError ? (error.fields ?? {}) : {});
    },
  });

  function submit(event: FormEvent) {
    event.preventDefault();
    setFields({});
    mutation.mutate();
  }

  return (
    <form className="space-y-4" onSubmit={submit}>
      {mode === "register" ? (
        <Field label="Name" error={fields.display_name}>
          <Input value={displayName} onChange={(event) => setDisplayName(event.target.value)} placeholder="Andras Bacsai" />
        </Field>
      ) : null}
      <Field label="Email" error={fields.email}>
        <Input type="email" required value={email} onChange={(event) => setEmail(event.target.value)} placeholder="you@coollabs.io" />
      </Field>
      <Field label="Password" hint={mode === "register" ? "12+ characters" : undefined} error={fields.password}>
        <Input type="password" required value={password} onChange={(event) => setPassword(event.target.value)} />
      </Field>
      <ErrorText error={mutation.error} />
      <Button className="w-full" variant="primary" type="submit" disabled={mutation.isPending}>
        {mutation.isPending ? "Working…" : mode === "login" ? "Sign in" : "Create account"}
      </Button>
    </form>
  );
}
