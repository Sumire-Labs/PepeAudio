/** Shared UI primitives and inline SVG icons (no icon dependency, CSP-clean). */
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(' ');
}

export function formatMs(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return '--:--';
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

type IconProps = { className?: string; style?: CSSProperties };
const svg = (children: ReactNode, filled = false) =>
  function Icon({ className, style }: IconProps) {
    return (
      <svg
        viewBox="0 0 24 24"
        className={className}
        style={style}
        fill={filled ? 'currentColor' : 'none'}
        stroke={filled ? 'none' : 'currentColor'}
        strokeWidth={filled ? 0 : 1.8}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        {children}
      </svg>
    );
  };

export const Icons = {
  Play: svg(<path d="M8 5.5v13l11-6.5z" />, true),
  Pause: svg(
    <>
      <rect x="6" y="5" width="4" height="14" rx="1.2" />
      <rect x="14" y="5" width="4" height="14" rx="1.2" />
    </>,
    true,
  ),
  Next: svg(<path d="M6 5l9 7-9 7zM17 5h2v14h-2z" />, true),
  Prev: svg(<path d="M18 5l-9 7 9 7zM5 5h2v14H5z" />, true),
  Stop: svg(<rect x="6" y="6" width="12" height="12" rx="2.2" />, true),
  Shuffle: svg(
    <>
      <path d="M4 5h4l10 14h2" />
      <path d="M18 5h2v0" />
      <path d="M16 5h4v4" />
      <path d="M4 19h4l3-4.2" />
      <path d="M16 19h4v-4" />
      <path d="M13.2 9.2 8 5H4" />
    </>,
  ),
  Repeat: svg(
    <>
      <path d="M17 3l3 3-3 3" />
      <path d="M4 12V9a3 3 0 0 1 3-3h13" />
      <path d="M7 21l-3-3 3-3" />
      <path d="M20 12v3a3 3 0 0 1-3 3H4" />
    </>,
  ),
  RepeatOne: svg(
    <>
      <path d="M17 3l3 3-3 3" />
      <path d="M4 12V9a3 3 0 0 1 3-3h13" />
      <path d="M7 21l-3-3 3-3" />
      <path d="M20 12v3a3 3 0 0 1-3 3H4" />
      <path d="M11 10.5h1.5V15" strokeWidth={1.6} />
    </>,
  ),
  Radio: svg(
    <>
      <circle cx="12" cy="12" r="2.2" fill="currentColor" stroke="none" />
      <path d="M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7" />
      <path d="M6 6a9 9 0 0 0 0 12M18 6a9 9 0 0 1 0 12" />
    </>,
  ),
  Plus: svg(
    <>
      <path d="M12 5v14M5 12h14" />
    </>,
  ),
  Trash: svg(
    <>
      <path d="M4 7h16" />
      <path d="M9 7V5h6v2" />
      <path d="M6 7l1 13h10l1-13" />
    </>,
  ),
  Grip: svg(
    <>
      <circle cx="9" cy="6" r="1.3" fill="currentColor" stroke="none" />
      <circle cx="9" cy="12" r="1.3" fill="currentColor" stroke="none" />
      <circle cx="9" cy="18" r="1.3" fill="currentColor" stroke="none" />
      <circle cx="15" cy="6" r="1.3" fill="currentColor" stroke="none" />
      <circle cx="15" cy="12" r="1.3" fill="currentColor" stroke="none" />
      <circle cx="15" cy="18" r="1.3" fill="currentColor" stroke="none" />
    </>,
  ),
  Queue: svg(
    <>
      <path d="M4 7h11M4 12h11M4 17h7" />
      <circle cx="18" cy="16" r="2.4" />
      <path d="M20.4 15V9l-2 .6" />
    </>,
  ),
  Playlist: svg(
    <>
      <path d="M4 6h10M4 10h10M4 14h6" />
      <circle cx="17" cy="15" r="3.2" />
      <path d="M20.2 14V7l-3 .8" />
    </>,
  ),
  Server: svg(
    <>
      <rect x="4" y="5" width="16" height="6" rx="2" />
      <rect x="4" y="13" width="16" height="6" rx="2" />
      <circle cx="8" cy="8" r="0.9" fill="currentColor" stroke="none" />
      <circle cx="8" cy="16" r="0.9" fill="currentColor" stroke="none" />
    </>,
  ),
  Logout: svg(
    <>
      <path d="M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3" />
      <path d="M10 8l-4 4 4 4" />
      <path d="M6 12h11" />
    </>,
  ),
  Headphones: svg(
    <>
      <path d="M5 13v-1a7 7 0 0 1 14 0v1" />
      <rect x="3.5" y="13" width="4" height="7" rx="1.6" />
      <rect x="16.5" y="13" width="4" height="7" rx="1.6" />
    </>,
  ),
  Spatial: svg(
    <>
      <circle cx="12" cy="12" r="8" />
      <ellipse cx="12" cy="12" rx="8" ry="3.4" />
      <ellipse cx="12" cy="12" rx="3.4" ry="8" />
    </>,
  ),
  Volume: svg(
    <>
      <path d="M5 9v6h3l4 4V5L8 9z" fill="currentColor" stroke="none" />
      <path d="M15.5 9.5a3.5 3.5 0 0 1 0 5M18 7a7 7 0 0 1 0 10" />
    </>,
  ),
  VolumeMute: svg(
    <>
      <path d="M5 9v6h3l4 4V5L8 9z" fill="currentColor" stroke="none" />
      <path d="M16 9.5l4 5M20 9.5l-4 5" />
    </>,
  ),
  Search: svg(
    <>
      <circle cx="11" cy="11" r="6.5" />
      <path d="M20 20l-3.6-3.6" />
    </>,
  ),
  Check: svg(<path d="M5 12.5l4.5 4.5L19 7" />),
  ChevronDown: svg(<path d="M6 9l6 6 6-6" />),
  Close: svg(<path d="M6 6l12 12M18 6L6 18" />),
  Menu: svg(<path d="M4 7h16M4 12h16M4 17h16" />),
  More: svg(
    <>
      <circle cx="5" cy="12" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="19" cy="12" r="1.7" fill="currentColor" stroke="none" />
    </>,
  ),
  Bookmark: svg(<path d="M6 4h12v16l-6-4-6 4z" />),
  Download: svg(
    <>
      <path d="M12 4v10" />
      <path d="M8 11l4 4 4-4" />
      <path d="M5 19h14" />
    </>,
  ),
  Edit: svg(
    <>
      <path d="M4 20h4l10-10-4-4L4 16v4z" />
      <path d="M13.5 6.5l4 4" />
    </>,
  ),
  Pin: svg(
    <>
      <path d="M9 4h6l-1 6 3 3H7l3-3-1-6z" />
      <path d="M12 16v4" />
    </>,
  ),
  Clock: svg(
    <>
      <circle cx="12" cy="12" r="8" />
      <path d="M12 8v4.5l3 2" />
    </>,
  ),
  Sun: svg(
    <>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4 12H2M22 12h-2M5 5l1.5 1.5M17.5 17.5 19 19M19 5l-1.5 1.5M6.5 17.5 5 19" />
    </>,
  ),
  Moon: svg(<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z" />),
};

export function Spinner({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={cx('spin', className)} fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeOpacity="0.2" strokeWidth="3" />
      <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}

/** Three animated bars — the "now playing" indicator. */
export function EqualizerBars({ className }: { className?: string }) {
  return (
    <span className={cx('inline-flex items-end gap-[2px]', className)} aria-hidden="true">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="w-[3px] rounded-full accent-bg"
          style={{
            height: 14,
            transformOrigin: 'bottom',
            animation: `equalize ${0.7 + i * 0.18}s ease-in-out ${i * 0.12}s infinite`,
          }}
        />
      ))}
    </span>
  );
}

export function IconButton({
  icon: IconComp,
  label,
  onClick,
  active,
  disabled,
  size = 'md',
  className,
}: {
  icon: (p: IconProps) => ReactNode;
  label: string;
  onClick?: () => void;
  active?: boolean;
  disabled?: boolean;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}) {
  const dim = size === 'lg' ? 'h-12 w-12' : size === 'sm' ? 'h-8 w-8' : 'h-10 w-10';
  const icon = size === 'lg' ? 'h-6 w-6' : size === 'sm' ? 'h-4 w-4' : 'h-5 w-5';
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      disabled={disabled}
      className={cx(
        'inline-grid place-items-center rounded-full transition-all duration-200 active:scale-90',
        dim,
        active ? 'accent' : 'text-[var(--text)]',
        disabled ? 'opacity-30' : 'hover:bg-[var(--track-bg)]',
        className,
      )}
    >
      <IconComp className={icon} />
      {active ? <span className="absolute -bottom-1 h-1 w-1 rounded-full accent-bg" /> : null}
    </button>
  );
}

/** A custom acrylic dropdown (replaces the native <select>, which can't be themed). */
export function Dropdown({
  value,
  options,
  disabled,
  onChange,
  icon: LeadIcon,
  className,
}: {
  value: string;
  options: Array<{ value: string; label: string }>;
  disabled?: boolean;
  onChange: (value: string) => void;
  icon?: (p: { className?: string }) => ReactNode;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  const current = options.find((o) => o.value === value);
  return (
    <div ref={ref} className={cx('relative', className)}>
      <button
        type="button"
        disabled={disabled}
        onClick={() => setOpen((v) => !v)}
        className="glass flex items-center gap-2 rounded-full px-3.5 py-2 text-sm font-medium text-[var(--text)] transition active:scale-95 disabled:opacity-40"
      >
        {LeadIcon ? <LeadIcon className="h-4 w-4 flex-none text-[var(--text-dim)]" /> : null}
        <span className="truncate">{current?.label ?? '—'}</span>
        <Icons.ChevronDown className={cx('h-4 w-4 flex-none text-[var(--text-dim)] transition-transform', open ? 'rotate-180' : '')} />
      </button>
      {open ? (
        <div
          className="glass-strong absolute left-0 top-full z-30 mt-2 max-h-60 min-w-[12rem] overflow-y-auto soft-scroll rounded-2xl p-1 fade-in"
          style={{ boxShadow: '0 16px 44px var(--shadow)' }}
        >
          {options.map((o) => (
            <button
              key={o.value}
              onClick={() => {
                onChange(o.value);
                setOpen(false);
              }}
              className={cx(
                'flex w-full items-center justify-between gap-3 rounded-xl px-3 py-2 text-left text-sm transition hover:bg-[var(--track-bg)]',
                o.value === value ? 'accent font-medium' : 'text-[var(--text)]',
              )}
            >
              <span className="truncate">{o.label}</span>
              {o.value === value ? <Icons.Check className="h-4 w-4 flex-none" /> : null}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

export interface MenuItem {
  label: string;
  icon?: (p: { className?: string }) => ReactNode;
  danger?: boolean;
  disabled?: boolean;
  onClick: () => void;
}

/** An overflow "⋯" button that opens an acrylic action menu (outside-click / Esc to close). */
export function Menu({ items, label = 'その他', align = 'right' }: { items: MenuItem[]; label?: string; align?: 'left' | 'right' }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        aria-label={label}
        title={label}
        onClick={() => setOpen((v) => !v)}
        className="glass grid h-9 w-9 place-items-center rounded-full text-[var(--text-dim)] transition hover:text-[var(--text)] active:scale-90"
      >
        <Icons.More className="h-5 w-5" />
      </button>
      {open ? (
        <div
          className={cx('glass-strong absolute top-full z-30 mt-2 min-w-[13rem] rounded-2xl p-1 fade-in', align === 'right' ? 'right-0' : 'left-0')}
          style={{ boxShadow: '0 16px 44px var(--shadow)' }}
        >
          {items.map((it) => (
            <button
              key={it.label}
              disabled={it.disabled}
              onClick={() => {
                it.onClick();
                setOpen(false);
              }}
              className={cx(
                'flex w-full items-center gap-2.5 rounded-xl px-3 py-2 text-left text-sm transition hover:bg-[var(--track-bg)] disabled:opacity-40',
                it.danger ? 'text-[var(--text)] hover:accent' : 'text-[var(--text)]',
              )}
            >
              {it.icon ? <it.icon className="h-4 w-4 flex-none text-[var(--text-dim)]" /> : null}
              {it.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
