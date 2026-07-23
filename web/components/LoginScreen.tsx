// SPDX-License-Identifier: Apache-2.0
"use client";
import { api } from "@/lib/api";
import { Icons } from "@/components/ui";

export function LoginScreen() {
  return (
    <div className="relative grid min-h-screen place-items-center px-6">
      <div
        className="glass-strong w-full max-w-sm rounded-[28px] p-8 text-center fade-in"
        style={{ boxShadow: "0 30px 80px var(--shadow)" }}
      >
        <div className="mx-auto mb-6 grid h-16 w-16 place-items-center rounded-2xl accent-bg text-white shadow-lg">
          <Icons.Headphones className="h-8 w-8" />
        </div>
        <h1 className="text-2xl font-semibold tracking-tight">PepeAudio</h1>
        <p className="mt-2 text-sm text-[var(--text-dim)]">Discordの音楽をWebから操作。</p>

        <a
          href={api.loginUrl}
          className="mt-7 flex w-full items-center justify-center gap-2 rounded-2xl accent-bg px-5 py-3 font-medium text-white transition-transform duration-150 hover:brightness-110 active:scale-[0.98]"
        >
          <Icons.Discord className="h-5 w-5" />
          Discordでログイン
        </a>
      </div>
    </div>
  );
}
