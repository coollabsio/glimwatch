import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Button, Card, Empty, ErrorText, Field, Input, Textarea } from "@/components/ui";
import { ApiError, api, type Monitor, type StatusPage } from "@/lib/api";

export function PagesPage() {
  const query = useQuery({ queryKey: ["pages"], queryFn: () => api<StatusPage[]>("/api/pages") });
  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Status pages</h1>
          <p className="text-sm text-muted-foreground">Public pages at /status/your-slug.</p>
        </div>
        <Link to="/pages/new"><Button>New page</Button></Link>
      </div>
      {query.isLoading ? <p className="text-sm text-muted-foreground">Loading pages…</p> : null}
      {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
        <Empty title="No status pages" body="Group monitors onto a page your customers can open without signing in." action={<Link to="/pages/new"><Button>New page</Button></Link>} />
      ) : (
        <div className="grid gap-3">
          {query.data?.map((page) => (
            <Card key={page.id} className="flex flex-wrap items-center justify-between gap-3 p-4">
              <div>
                <Link className="font-medium hover:text-primary" to={`/pages/${page.id}`}>{page.title}</Link>
                <div className="text-sm text-muted-foreground">/{page.slug} · {page.components.length} components · {page.published ? "published" : "draft"}</div>
              </div>
              {page.published ? <Link className="text-sm text-primary" to={`/status/${page.slug}`}>View public page</Link> : null}
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

export function PageEditor() {
  const { id = "new" } = useParams();
  const creating = id === "new";
  const navigate = useNavigate();
  const client = useQueryClient();
  const existing = useQuery({
    queryKey: ["page", id],
    queryFn: () => api<StatusPage>(`/api/pages/${id}`),
    enabled: !creating,
  });
  const monitors = useQuery({ queryKey: ["monitors"], queryFn: () => api<Monitor[]>("/api/monitors") });
  const page = existing.data;
  const [title, setTitle] = useState(page?.title ?? "");
  const [description, setDescription] = useState(page?.description ?? "");
  const [slug, setSlug] = useState(page?.slug ?? "");
  const [homepage, setHomepage] = useState(page?.homepage_url ?? "");
  const [published, setPublished] = useState(page?.published ?? true);
  const [selected, setSelected] = useState<string[]>(page?.components.map((component) => component.monitor_id) ?? []);
  const [fields, setFields] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!page) return;
    setTitle(page.title);
    setDescription(page.description);
    setSlug(page.slug);
    setHomepage(page.homepage_url ?? "");
    setPublished(page.published);
    setSelected(page.components.map((component) => component.monitor_id));
  }, [page]);

  const save = useMutation({
    mutationFn: () =>
      api<StatusPage>(creating ? "/api/pages" : `/api/pages/${id}`, {
        method: creating ? "POST" : "PUT",
        body: JSON.stringify({
          title,
          description,
          slug,
          homepage_url: homepage,
          published,
          monitor_ids: selected,
        }),
      }),
    onSuccess: async (saved) => {
      await client.invalidateQueries({ queryKey: ["pages"] });
      navigate(`/pages/${saved.id}`);
    },
    onError: (error) => setFields(error instanceof ApiError ? error.fields ?? {} : {}),
  });
  const remove = useMutation({
    mutationFn: () => api(`/api/pages/${id}`, { method: "DELETE" }),
    onSuccess: () => navigate("/pages"),
  });

  function submit(event: FormEvent) {
    event.preventDefault();
    setFields({});
    save.mutate();
  }

  if (!creating && existing.isLoading) return <p className="text-sm text-muted-foreground">Loading page…</p>;

  return (
    <form className="mx-auto max-w-2xl space-y-4" onSubmit={submit}>
      <h1 className="text-2xl font-semibold tracking-tight">{creating ? "New status page" : "Edit status page"}</h1>
      <Field label="Title" error={fields.title}><Input value={title} onChange={(event) => setTitle(event.target.value)} required /></Field>
      <Field label="Description" error={fields.description}><Textarea value={description} onChange={(event) => setDescription(event.target.value)} /></Field>
      <Field label="Slug" hint="public URL" error={fields.slug}><Input value={slug} onChange={(event) => setSlug(event.target.value)} placeholder="coollabs" required /></Field>
      <Field label="Homepage" error={fields.homepage_url}><Input value={homepage} onChange={(event) => setHomepage(event.target.value)} placeholder="https://coolify.io" /></Field>
      <label className="flex items-center gap-2 text-sm">
        <input type="checkbox" checked={published} onChange={(event) => setPublished(event.target.checked)} />
        Published
      </label>
      <div>
        <div className="mb-2 text-sm font-medium">Monitors on this page</div>
        <div className="space-y-2 rounded-xl border p-3">
          {(monitors.data ?? []).length === 0 ? <p className="text-sm text-muted-foreground">Create a monitor first.</p> : null}
          {monitors.data?.map((monitor) => (
            <label key={monitor.id} className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={selected.includes(monitor.id)}
                onChange={(event) =>
                  setSelected(event.target.checked ? [...selected, monitor.id] : selected.filter((item) => item !== monitor.id))
                }
              />
              {monitor.name}
            </label>
          ))}
        </div>
        {fields.monitor_ids ? <p className="mt-1 text-xs text-destructive">{fields.monitor_ids}</p> : null}
      </div>
      <ErrorText error={save.error || remove.error} />
      <div className="flex justify-between gap-2">
        {!creating ? <Button type="button" variant="destructive" onClick={() => remove.mutate()}>Delete</Button> : <span />}
        <Button type="submit" disabled={save.isPending}>{save.isPending ? "Saving…" : "Save page"}</Button>
      </div>
      {!creating && page?.published ? (
        <p className="text-sm text-muted-foreground">
          Public URL: <Link className="text-primary" to={`/status/${page.slug}`}>/status/{page.slug}</Link>
        </p>
      ) : null}
    </form>
  );
}
