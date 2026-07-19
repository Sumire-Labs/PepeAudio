// CSS blur of the <img>, not canvas pixel reads, so cross-origin CDN artwork needs no CORS.
export function Ambient({ url }: { url: string | null }) {
  return (
    <div className="pointer-events-none fixed inset-0 -z-10 overflow-hidden" aria-hidden="true">
      <div className="absolute inset-0" style={{ background: 'var(--bg)' }} />
      {url ? (
        <img
          key={url}
          src={url}
          alt=""
          className="absolute inset-0 h-full w-full object-cover fade-in"
          style={{ filter: 'blur(90px) saturate(1.9)', transform: 'scale(1.5)', opacity: 0.6 }}
        />
      ) : null}
      <div
        className="absolute inset-0"
        style={{ background: 'color-mix(in srgb, var(--bg) 42%, transparent)' }}
      />
      <div
        className="absolute inset-0"
        style={{ background: 'linear-gradient(to bottom, transparent 0%, color-mix(in srgb, var(--bg) 70%, transparent) 100%)' }}
      />
    </div>
  );
}
