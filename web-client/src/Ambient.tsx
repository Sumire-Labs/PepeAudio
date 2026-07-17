/**
 * The Apple Music-style ambient background: the current artwork, scaled up and
 * heavily blurred, cross-fading on track change. Pure CSS blur of the <img> — no
 * canvas pixel reads, so cross-origin CDN artwork works without CORS.
 */
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
      {/* Tone the bloom into the theme background so text stays legible in light & dark. */}
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
