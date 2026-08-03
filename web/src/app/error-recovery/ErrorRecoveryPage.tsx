export function ErrorRecoveryPage({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  const reloadHref = typeof window === "undefined" ? "/" : window.location.href;

  return (
    <main className="grid min-h-dvh place-items-center bg-surface px-5 py-10 text-primary">
      <section
        className="w-full max-w-lg rounded-panel border border-subtle bg-surface-muted p-7 shadow-panel"
        role="alert"
      >
        <p className="text-[12px] font-medium tracking-[0.14em] text-tertiary">ANY2API</p>
        <h1 className="mt-3 text-xl font-semibold tracking-tight">{title}</h1>
        <p className="mt-2 text-sm leading-6 text-secondary">{description}</p>
        <a
          className="focus-ring mt-6 inline-flex min-h-9 items-center justify-center rounded-[8px] bg-accent px-4 text-sm font-semibold text-on-accent"
          href={reloadHref}
        >
          重新加载
        </a>
      </section>
    </main>
  );
}
