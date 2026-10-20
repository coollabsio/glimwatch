import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, type ReactNode, useState } from "react";
import { Link, useNavigate } from "react-router";
import { Mark } from "@/app/Shell";
import { Button, Card, ErrorText, Field, Input } from "@/components/ui";
import { ApiError, api, type Meta, type User } from "@/lib/api";

export function LoginPage() {
  return (
    <AuthShell
      title="Sign in"
      subtitle="Status pages and uptime for the coolLabs stack."
      footer={
        <>
          Need an account? <Link className="text-primary" to="/register">Create one</Link>
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
      subtitle="The first account becomes the owner of this CoolStatus install."
      footer={
        <>
          Already registered? <Link className="text-primary" to="/login">Sign in</Link>
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
    <div className="grid min-h-svh place-items-center px-4 py-10">
      <div className="w-full max-w-md">
        <div className="mb-6 flex items-center gap-3">
          <Mark />
          <div>
            <div className="font-semibold">CoolStatus</div>
            <div className="text-xs text-muted-foreground">by coolLabs</div>
          </div>
        </div>
        <Card className="p-6">
          <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>
          <div className="mt-6">{children}</div>
        </Card>
        {meta.data?.demo ? (
          <p className="mt-4 text-center text-xs text-muted-foreground">
            Seeded demo: {meta.data.demo.email} / {meta.data.demo.password}
          </p>
        ) : null}
        <p className="mt-3 text-center text-sm text-muted-foreground">{footer}</p>
      </div>
    </div>
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
      setFields(error instanceof ApiError ? error.fields ?? {} : {});
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
      <Button className="w-full" type="submit" disabled={mutation.isPending}>
        {mutation.isPending ? "Working…" : mode === "login" ? "Sign in" : "Create account"}
      </Button>
    </form>
  );
}
