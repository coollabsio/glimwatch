import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Button, Empty, ErrorText, Field, Input, LayerCard, PageHeader, Textarea } from "@/components/ui";
import { ApiError, api, type Monitor, type StatusPage } from "@/lib/api";

export function PagesPage() {
  const query = useQuery({ queryKey: ["pages"], queryFn: () => api<StatusPage[]>("/api/pages") });
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Status pages"
        description="Public pages at /status/your-slug."
        actions={<Link to="/pages/new"><Button variant="primary">New page</Button></Link>}
      />
      {query.isLoading ? <p className="text-[13px] text-[var(--coollabs-subtle)]">Loading pages…</p> : null}
      {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
        <Empty title="No status pages" body="Group monitors onto a page your customers can open without signing in." action={<Link to="/pages/new"><Button variant="primary">New page</Button></Link>} />
      ) : (
        <div className="flex flex-col gap-4">
          {query.data?.map((page) => (
            <LayerCard
              key={page.id}
              title={page.title}
              description={`/${page.slug} · ${page.components.length} components · ${page.published ? "published" : "draft"}`}
              actions={
                <>
                  <Link to={`/pages/${page.id}`}><Button>Edit</Button></Link>
                  {page.published ? <Link to={`/status/${page.slug}`}><Button variant="primary">View</Button></Link> : null}
                </>
              }
            >
              <p className="text-[13px] text-[var(--coollabs-subtle)]">Customers open this page without signing in.</p>
            </LayerCard>
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
    <form className="mx-auto flex max-w-2xl flex-col gap-4" onSubmit={submit}>
      <PageHeader title={creating ? "New status page" : "Edit status page"} description="Choose a slug and the monitors that appear on it." />
      <Field label="Title" error={fields.title}><Input value={title} onChange={(event) => setTitle(event.target.value)} required /></Field>
      <Field label="Description" error={fields.description}><Textarea value={description} onChange={(event) => setDescription(event.target.value)} /></Field>
      <Field label="Slug" hint="public URL" error={fields.slug}><Input value={slug} onChange={(event) => setSlug(event.target.value)} placeholder="coollabs" required /></Field>
      <Field label="Homepage" error={fields.homepage_url}><Input value={homepage} onChange={(event) => setHomepage(event.target.value)} placeholder="https://coolify.io" /></Field>
      <label className="check-row">
        <input type="checkbox" checked={published} onChange={(event) => setPublished(event.target.checked)} />
        Published
      </label>
      <div>
        <div className="field-label">Monitors on this page</div>
        <div className="space-y-2 rounded-lg bg-[var(--coollabs-recessed)] p-3 shadow-[0_0_0_1px_var(--coollabs-line)]">
          {(monitors.data ?? []).length === 0 ? <p className="text-sm text-muted-foreground">Create a monitor first.</p> : null}
          {monitors.data?.map((monitor) => (
            <label key={monitor.id} className="check-row">
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
        {!creating ? <Button type="button" variant="danger" onClick={() => remove.mutate()}>Delete</Button> : <span />}
        <Button type="submit" variant="primary" disabled={save.isPending}>{save.isPending ? "Saving…" : "Save page"}</Button>
      </div>
      {!creating && page?.published ? (
        <p className="text-[13px] text-[var(--coollabs-subtle)]">
          Public URL: <Link className="auth-text-link" to={`/status/${page.slug}`}>/status/{page.slug}</Link>
        </p>
      ) : null}
    </form>
  );
}
