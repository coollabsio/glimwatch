import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Button, Empty, ErrorText, Field, Input, LayerCard, PageHeader, Select, StatusBadge, Textarea } from "@/components/ui";
import { ApiError, api, type Monitor, type StatusPage } from "@/lib/api";

export function PagesPage() {
  const query = useQuery({ queryKey: ["pages"], queryFn: () => api<StatusPage[]>("/api/pages") });
  const [search, setSearch] = useState("");
  const needle = search.trim().toLowerCase();
  const pages = (query.data ?? []).filter((page) => page.title.toLowerCase().includes(needle) || page.slug.toLowerCase().includes(needle));
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Status pages"
        description="Public pages at /status/your-slug."
        actions={<Link to="/pages/new"><Button variant="highlighted">New page</Button></Link>}
      />
      {query.isLoading ? (
        <p className="text-[13px] text-[var(--ui-fg-faint)]">Loading pages…</p>
      ) : (query.data?.length ?? 0) === 0 ? (
        <Empty title="No status pages" body="Group monitors onto a page your customers can open without signing in." action={<Link to="/pages/new"><Button variant="highlighted">New page</Button></Link>} />
      ) : (
        <LayerCard title="All pages" description="Customers open these pages without signing in." flush>
          <div className="flex items-center gap-2 border-b border-[var(--ui-divider)] p-2">
            <Input className="max-w-64" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search pages" aria-label="Search pages" />
          </div>
          <div className="overflow-x-auto">
            <table className="cool-table">
              <thead>
                <tr>
                  <th>Title</th>
                  <th className="hidden sm:table-cell">Slug</th>
                  <th className="hidden md:table-cell">Components</th>
                  <th>Status</th>
                  <th className="w-0" />
                </tr>
              </thead>
              <tbody>
                {pages.map((page) => (
                  <tr key={page.id}>
                    <td><Link className="row-link" to={`/pages/${page.id}`}>{page.title}</Link></td>
                    <td className="hidden font-mono text-[12px] sm:table-cell">/{page.slug}</td>
                    <td className="hidden md:table-cell">{page.components.length}</td>
                    <td><StatusBadge tone={page.published ? "success" : "neutral"}>{page.published ? "Published" : "Draft"}</StatusBadge></td>
                    <td>
                      <div className="flex justify-end gap-2">
                        <Link to={`/pages/${page.id}`}><Button>Edit</Button></Link>
                        {page.published ? <Link to={`/status/${page.slug}`}><Button>View</Button></Link> : null}
                      </div>
                    </td>
                  </tr>
                ))}
                {pages.length === 0 ? (
                  <tr>
                    <td colSpan={5} className="text-center text-[var(--ui-fg-faint)]">No pages match your search.</td>
                  </tr>
                ) : null}
              </tbody>
            </table>
          </div>
        </LayerCard>
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

  if (!creating && existing.isLoading) return <p className="text-[13px] text-[var(--ui-fg-faint)]">Loading page…</p>;

  return (
    <form className="mx-auto flex w-full max-w-3xl flex-col gap-6" onSubmit={submit}>
      <PageHeader
        title={creating ? "New status page" : "Edit status page"}
        description="Choose a slug and the monitors that appear on it."
        actions={!creating && page?.published ? <Link to={`/status/${page.slug}`}><Button type="button">View public page</Button></Link> : null}
      />
      <LayerCard title="Details" description={!creating && page?.published ? `Public URL: /status/${page.slug}` : "What customers see at the top of the page."}>
        <div className="flex flex-col gap-4">
          <Field label="Title" error={fields.title}><Input value={title} onChange={(event) => setTitle(event.target.value)} required /></Field>
          <Field label="Description" error={fields.description}><Textarea value={description} onChange={(event) => setDescription(event.target.value)} /></Field>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Slug" hint="public URL" error={fields.slug}><Input value={slug} onChange={(event) => setSlug(event.target.value)} placeholder="coollabs" required /></Field>
            <Field label="Homepage" error={fields.homepage_url}><Input value={homepage} onChange={(event) => setHomepage(event.target.value)} placeholder="https://coolify.io" /></Field>
          </div>
          <Field label="Visibility">
            <Select
              value={published ? "published" : "draft"}
              options={[
                { value: "published", label: "Published" },
                { value: "draft", label: "Draft" },
              ]}
              onChange={(value) => setPublished(value === "published")}
              aria-label="Visibility"
            />
          </Field>
        </div>
      </LayerCard>
      <LayerCard title="Monitors" description="Each selected monitor shows as a component on the page.">
        {(monitors.data ?? []).length === 0 ? (
          <p className="py-4 text-center text-[13px] text-[var(--ui-fg-faint)]">Create a monitor first.</p>
        ) : (
          <div className="grid gap-2 sm:grid-cols-2">
            {monitors.data?.map((monitor) => (
              <label key={monitor.id} className="check-row">
                <input
                  type="checkbox"
                  className="size-4 accent-[var(--ui-accent)]"
                  checked={selected.includes(monitor.id)}
                  onChange={(event) =>
                    setSelected(event.target.checked ? [...selected, monitor.id] : selected.filter((item) => item !== monitor.id))
                  }
                />
                {monitor.name}
              </label>
            ))}
          </div>
        )}
        {fields.monitor_ids ? <p className="field-error">{fields.monitor_ids}</p> : null}
      </LayerCard>
      <ErrorText error={save.error || remove.error} />
      <div className="flex justify-between gap-2">
        {!creating ? <Button type="button" variant="danger" onClick={() => remove.mutate()}>Delete</Button> : <span />}
        <Button type="submit" variant="highlighted" disabled={save.isPending}>{save.isPending ? "Saving…" : "Save page"}</Button>
      </div>
    </form>
  );
}
