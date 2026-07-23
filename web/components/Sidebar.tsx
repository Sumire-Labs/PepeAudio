// SPDX-License-Identifier: Apache-2.0
"use client";
/** Collapsible left rail: nav, guild list, and the user/theme footer. */
import { useEffect, useState } from "react";
import { useTheme } from "next-themes";
import { cx, Icons, type Icon } from "@/components/ui";
import { guildIconUrl, userAvatarUrl } from "@/lib/format";
import type { Guild, Me } from "@/lib/types";

export function Sidebar({
  me,
  guilds,
  selectedGuildId,
  view,
  open,
  onSelectGuild,
  onView,
  onCloseSidebar,
  onLogout,
}: {
  me: Me;
  guilds: Guild[];
  selectedGuildId: string | null;
  view: "player" | "playlists";
  open: boolean;
  onSelectGuild: (id: string) => void;
  onView: (v: "player" | "playlists") => void;
  onCloseSidebar: () => void;
  onLogout: () => void;
}) {
  const [railHover, setRailHover] = useState(false);
  // SSR-safe: start desktop=false so server and first client paint agree; the effect corrects it.
  const [isDesktop, setIsDesktop] = useState(false);
  useEffect(() => {
    const mq = window.matchMedia("(min-width: 768px)");
    const onChange = () => setIsDesktop(mq.matches);
    onChange();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const { theme, setTheme } = useTheme();
  // next-themes resolves on the client only — gate the icon on mount to dodge an SSR mismatch.
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const cycleTheme = () => setTheme(theme === "system" ? "dark" : theme === "dark" ? "light" : "system");
  const ThemeIcon = theme === "light" ? Icons.Sun : theme === "dark" ? Icons.Moon : Icons.Spatial;
  const themeLabel = theme === "light" ? "ライト" : theme === "dark" ? "ダーク" : "自動";

  const avatarUrl = userAvatarUrl(me.id, me.avatar);

  return (
    <aside
      onMouseEnter={() => setRailHover(true)}
      onMouseLeave={() => setRailHover(false)}
      className={cx(
        // Width is set inline on desktop to beat the class cascade; falls back to the CSS drawer width on mobile.
        "glass-strong side-rail fixed left-0 top-0 z-40 flex h-full flex-col overflow-hidden p-3 transition-[width,transform] duration-300 ease-out",
        open ? "translate-x-0" : "-translate-x-full",
        "md:translate-x-0",
        railHover ? "is-open" : "",
      )}
      style={{ boxShadow: "0 0 40px var(--shadow)", ...(isDesktop ? { width: railHover ? "15.5rem" : "4.5rem" } : {}) }}
    >
      <header className="rail-center mb-3 flex h-9 items-center gap-2.5">
        <div className="grid h-9 w-9 flex-none place-items-center rounded-xl accent-bg text-white">
          <Icons.Headphones className="h-5 w-5" />
        </div>
        <span className="rail-label whitespace-nowrap text-lg font-semibold tracking-tight">PepeAudio</span>
        <button
          onClick={onCloseSidebar}
          aria-label="閉じる"
          className="ml-auto grid h-8 w-8 flex-none place-items-center rounded-full text-[var(--text-dim)] hover:bg-[var(--track-bg)] md:hidden"
        >
          <Icons.Close className="h-4 w-4" />
        </button>
      </header>

      <nav className="mb-3 flex flex-col gap-1">
        <NavRow icon={Icons.Play} label="プレイヤー" active={view === "player"} onClick={() => onView("player")} />
        <NavRow icon={Icons.Playlist} label="プレイリスト" active={view === "playlists"} onClick={() => onView("playlists")} />
      </nav>

      <div className="rail-label mb-1 flex items-center justify-between px-2">
        <span className="whitespace-nowrap text-xs font-semibold uppercase tracking-wide text-[var(--text-faint)]">サーバー</span>
        <span className="text-xs text-[var(--text-faint)]">{guilds.length}</span>
      </div>
      {/* A thin divider that shows only in the collapsed rail (where the label row is hidden). */}
      <div className="rail-collapsed-only mb-1 hidden h-px bg-[var(--hairline)] md:block" />

      <div className="min-h-0 flex-1 space-y-0.5 overflow-y-auto soft-scroll">
        {guilds.length === 0 ? (
          <p className="rail-label whitespace-nowrap px-2 py-4 text-sm text-[var(--text-faint)]">共通のサーバーがありません。</p>
        ) : (
          guilds.map((g) => {
            const selected = selectedGuildId === g.id;
            const iconUrl = guildIconUrl(g.id, g.icon);
            return (
              <button
                key={g.id}
                onClick={() => onSelectGuild(g.id)}
                title={g.name}
                style={selected ? { background: "color-mix(in srgb, var(--accent) 20%, transparent)" } : undefined}
                className={cx(
                  "rail-center flex w-full items-center gap-2.5 rounded-xl p-1.5 text-left transition",
                  selected ? "" : "hover:bg-[var(--track-bg)]",
                )}
              >
                <div className="flex-none">
                  {iconUrl ? (
                    <img src={iconUrl} alt="" className="h-9 w-9 rounded-xl object-cover" />
                  ) : (
                    <div className="grid h-9 w-9 place-items-center rounded-xl bg-[var(--track-bg)] text-sm font-semibold text-[var(--text-dim)]">
                      {g.name.slice(0, 1).toUpperCase()}
                    </div>
                  )}
                </div>
                <span className={cx("rail-label min-w-0 flex-1 truncate text-sm font-medium", selected ? "accent" : "text-[var(--text)]")}>
                  {g.name}
                </span>
              </button>
            );
          })
        )}
      </div>

      <div className="rail-center mt-2 flex items-center gap-2 border-t border-[var(--hairline)] pt-2">
        {avatarUrl ? (
          <img src={avatarUrl} alt="" className="h-9 w-9 flex-none rounded-full object-cover" />
        ) : (
          <div className="grid h-9 w-9 flex-none place-items-center rounded-full bg-[var(--track-bg)] text-sm">
            {me.username.slice(0, 1).toUpperCase()}
          </div>
        )}
        <span className="rail-label min-w-0 flex-1 truncate text-sm">{me.username}</span>
        <div className="rail-label flex flex-none items-center gap-1">
          <button
            onClick={cycleTheme}
            aria-label={`テーマ: ${themeLabel}`}
            title={`テーマ: ${themeLabel}`}
            className="grid h-8 w-8 place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)]"
          >
            {mounted ? <ThemeIcon className="h-4 w-4" /> : <span className="h-4 w-4" />}
          </button>
          <button
            onClick={onLogout}
            aria-label="ログアウト"
            title="ログアウト"
            className="grid h-8 w-8 place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)] hover:accent"
          >
            <Icons.Logout className="h-4 w-4" />
          </button>
        </div>
      </div>
    </aside>
  );
}

function NavRow({ icon: IconComp, label, active, onClick }: { icon: Icon; label: string; active: boolean; onClick: () => void }) {
  return (
    <button onClick={onClick} title={label} className="rail-center flex items-center gap-2.5 rounded-xl p-1.5 text-left transition hover:bg-[var(--track-bg)]">
      <span className={cx("grid h-9 w-9 flex-none place-items-center rounded-xl transition", active ? "accent-bg text-white" : "bg-[var(--track-bg)] text-[var(--text-dim)]")}>
        <IconComp className="h-5 w-5" />
      </span>
      <span className={cx("rail-label whitespace-nowrap text-sm font-medium", active ? "text-[var(--text)]" : "text-[var(--text-dim)]")}>{label}</span>
    </button>
  );
}
