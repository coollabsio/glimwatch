import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, type ReactNode, useState } from "react";
import { Link, useNavigate } from "react-router";
import { Logo } from "@/app/Shell";
import { Button, ErrorText, Field, Input } from "@/components/ui";
import { ApiError, api, type Meta, type User } from "@/lib/api";

export function LoginPage() {
  return (
    <AuthShell
      title="Sign in"
      subtitle="Status pages and uptime for the coolLabs stack."
      footer={
        <>
          Need an account? <Link className="auth-text-link" to="/register">Create one</Link>
        </>
      }
    >
      <AuthForm mode="login" />
    </AuthShell>
  );
}

export function RegisterPage() {
  return (
    <AuthShell
      title="Create the operator account"
      subtitle="The first account becomes the owner of this Haruvo install."
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
            <footer className="auth-card-footer">{footer}</footer>
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
