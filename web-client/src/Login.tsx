import { useEffect, useState } from 'react';
import { Icons } from './ui.tsx';

/** Full-screen acrylic login card. The button is a plain link to the OAuth start. */
export function Login() {
  const [authError, setAuthError] = useState(false);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get('auth') === 'error') setAuthError(true);
    if (params.has('auth')) {
      // Clean the query so a refresh doesn't re-show the banner.
      window.history.replaceState({}, '', window.location.pathname);
    }
  }, []);

  return (
    <div className="relative grid min-h-full place-items-center px-6">
      <div className="glass-strong w-full max-w-sm rounded-[28px] p-8 text-center fade-in" style={{ boxShadow: '0 30px 80px var(--shadow)' }}>
        <div className="mx-auto mb-6 grid h-16 w-16 place-items-center rounded-2xl accent-bg text-white shadow-lg">
          <Icons.Headphones className="h-8 w-8" />
        </div>
        <h1 className="text-2xl font-semibold tracking-tight">PepeAudio</h1>
        <p className="mt-2 text-sm text-[var(--text-dim)]">
          Discord でログインして、ブラウザから再生をコントロール。
        </p>

        {authError ? (
          <p className="mt-4 rounded-xl bg-[var(--track-bg)] px-3 py-2 text-sm accent">
            ログインに失敗しました。もう一度お試しください。
          </p>
        ) : null}

        <a
          href="/auth/login"
          className="mt-7 flex w-full items-center justify-center gap-2 rounded-2xl accent-bg px-5 py-3 font-medium text-white transition-transform duration-150 hover:brightness-110 active:scale-[0.98]"
        >
          <DiscordMark />
          Discord でログイン
        </a>
        <p className="mt-4 text-xs text-[var(--text-faint)]">
          操作するには Bot と同じボイスチャンネルに参加している必要があります。
        </p>
      </div>
    </div>
  );
}

function DiscordMark() {
  return (
    <svg viewBox="0 0 24 24" className="h-5 w-5" fill="currentColor" aria-hidden="true">
      <path d="M19.3 5.4A17 17 0 0 0 15 4l-.2.4a13 13 0 0 1 3.7 1.8 12 12 0 0 0-10.9 0A13 13 0 0 1 11.2 4.4L11 4a17 17 0 0 0-4.3 1.4C3.9 9.6 3.1 13.7 3.5 17.7a17 17 0 0 0 5.2 2.6l.4-.6c-.6-.2-1.2-.5-1.7-.8l.4-.3a9 9 0 0 0 8.4 0l.4.3c-.5.3-1.1.6-1.7.8l.4.6a17 17 0 0 0 5.2-2.6c.5-4.7-.8-8.8-3.6-12.3ZM9.5 15.2c-.8 0-1.5-.8-1.5-1.7s.7-1.7 1.5-1.7 1.5.8 1.5 1.7-.7 1.7-1.5 1.7Zm5 0c-.8 0-1.5-.8-1.5-1.7s.7-1.7 1.5-1.7 1.5.8 1.5 1.7-.7 1.7-1.5 1.7Z" />
    </svg>
  );
}
