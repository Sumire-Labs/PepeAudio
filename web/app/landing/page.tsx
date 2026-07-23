// SPDX-License-Identifier: Apache-2.0
import type { ComponentType } from "react";
import { Headphones, Radio, Spatial, Server, Discord } from "@/components/icons";

type Feature = [ComponentType<{ className?: string }>, string, string];

const features: Feature[] = [
  [Headphones, "ネイティブ高音質オーディオ", "Aura HeSuVi プリセットとギャップレスクロスフェードを備えたネイティブパイプライン。Lavalink 不要。"],
  [Radio, "あらゆるプラットフォーム", "YouTube、SoundCloud、Spotify、Apple Music、直接リンク、ファイルアップロードに対応。"],
  [Spatial, "美しい操作性", "Discord では Components V2、さらに Apple Music 風のウェブリモコン。"],
  [Server, "スケールする設計", "初日からシャーディング、Valkey で連携 — 1,000 から 100,000 以上のサーバーまで。"],
];

export default function Landing() {
  return (
    <main className="mx-auto flex min-h-screen max-w-4xl flex-col items-center px-6 py-20 text-center fade-in">
      <div className="grid h-20 w-20 place-items-center rounded-3xl accent-bg text-white shadow-lg">
        <Headphones className="h-10 w-10" />
      </div>
      <h1 className="mt-8 text-5xl font-semibold tracking-tight">PepeAudio</h1>
      <p className="mt-4 max-w-xl text-lg text-[var(--text-dim)]">
        Discord 向けの高速・高音質な音楽 BOT。Apple Music のようなウェブリモコン付き。
      </p>
      <div className="mt-8 flex flex-wrap items-center justify-center gap-4">
        <a
          href="/api/auth/login"
          className="flex items-center gap-2 rounded-full accent-bg px-6 py-3 font-medium text-white transition-transform duration-150 hover:brightness-110 active:scale-[0.98]"
        >
          <Discord className="h-5 w-5" />
          プレイヤーを開く
        </a>
        <a
          href="https://github.com/Sumire-Labs/PepeAudio"
          className="glass rounded-full px-6 py-3 font-medium transition-transform duration-150 hover:brightness-110 active:scale-[0.98]"
        >
          GitHub
        </a>
      </div>

      <section className="mt-20 grid w-full gap-4 sm:grid-cols-2">
        {features.map(([Icon, title, body]) => (
          <div key={title} className="glass rounded-3xl p-6 text-left">
            <div className="grid h-11 w-11 place-items-center rounded-2xl bg-[var(--track-bg)] accent">
              <Icon className="h-6 w-6" />
            </div>
            <h2 className="mt-4 text-lg font-semibold">{title}</h2>
            <p className="mt-1 text-sm text-[var(--text-dim)]">{body}</p>
          </div>
        ))}
      </section>

      <footer className="mt-20 text-xs text-[var(--text-faint)]">
        オープンソース · Apache-2.0。各ソースの利用規約を遵守する責任はあなたにあります。
      </footer>
    </main>
  );
}
