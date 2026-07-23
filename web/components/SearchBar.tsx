// SPDX-License-Identifier: Apache-2.0
"use client";
import { useState } from "react";
import { Search } from "@/components/icons";

export function SearchBar({ onPlay }: { onPlay: (query: string) => void }) {
  const [value, setValue] = useState("");

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    const q = value.trim();
    if (!q) return;
    onPlay(q);
    setValue("");
  };

  return (
    <form onSubmit={submit} className="px-8 pt-6">
      <div className="glass flex items-center gap-3 rounded-2xl px-4 py-3">
        <Search className="h-5 w-5 flex-none text-[var(--text-dim)]" />
        <input
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="リンクを貼り付けるか検索 — YouTube、Spotify、Apple Music、SoundCloud…"
          className="w-full bg-transparent text-sm outline-none placeholder:text-[var(--text-faint)]"
        />
      </div>
    </form>
  );
}
