// SPDX-License-Identifier: Apache-2.0
"use client";
import { useEffect, useState } from "react";
import { api, UnauthorizedError } from "@/lib/api";
import type { PlaylistSummary, QueueItem } from "@/lib/types";
import { Icons, Spinner } from "@/components/ui";
import { toPlaylistTrack } from "@/lib/format";
import { useToast } from "@/components/toast";

export function AddToPlaylistModal({
  track,
  onClose,
  onAdded,
}: {
  track: QueueItem;
  onClose: () => void;
  onAdded: () => void;
}) {
  const toast = useToast();
  const [list, setList] = useState<PlaylistSummary[] | null>(null);
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .listPlaylists()
      .then(({ playlists }) => setList(playlists))
      .catch((err) => {
        if (err instanceof UnauthorizedError) location.reload();
        else setList([]);
      });
  }, []);

  const fail = (err: unknown) => {
    if (err instanceof UnauthorizedError) {
      location.reload();
      return;
    }
    toast(err instanceof Error ? err.message : "失敗しました。", "error");
    setBusy(false);
  };

  const addTo = async (id: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await api.addPlaylistTrack(id, toPlaylistTrack(track));
      toast("プレイリストに追加しました。");
      onAdded();
      onClose();
    } catch (err) {
      fail(err);
    }
  };

  const createAndAdd = async () => {
    const name = newName.trim();
    if (!name || busy) return;
    setBusy(true);
    try {
      const { playlist } = await api.createPlaylist(name);
      await api.addPlaylistTrack(playlist.id, toPlaylistTrack(track));
      toast(`「${name}」に追加しました。`);
      onAdded();
      onClose();
    } catch (err) {
      fail(err);
    }
  };

  return (
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/40 p-6 fade-in" onClick={onClose}>
      <div
        className="glass-strong w-full max-w-sm rounded-3xl p-5"
        onClick={(e) => e.stopPropagation()}
        style={{ boxShadow: "0 30px 80px var(--shadow)" }}
      >
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-lg font-semibold">プレイリストに保存</h3>
          <button
            onClick={onClose}
            aria-label="閉じる"
            className="grid h-8 w-8 place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)]"
          >
            <Icons.Close className="h-4 w-4" />
          </button>
        </div>
        <p className="mb-3 truncate text-sm text-[var(--text-dim)]">{track.title}</p>

        <div className="mb-3 flex gap-2">
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && createAndAdd()}
            placeholder="新規プレイリスト名"
            className="glass w-full rounded-xl px-3 py-2 text-sm outline-none"
          />
          <button
            onClick={createAndAdd}
            disabled={!newName.trim() || busy}
            className="rounded-xl accent-bg px-3 text-sm font-medium text-white transition active:scale-95 disabled:opacity-40"
          >
            作成
          </button>
        </div>

        <div className="max-h-64 space-y-1 overflow-y-auto soft-scroll">
          {list === null ? (
            <div className="grid place-items-center py-6">
              <Spinner className="h-6 w-6" />
            </div>
          ) : list.length === 0 ? (
            <p className="py-4 text-center text-sm text-[var(--text-faint)]">まだプレイリストがありません。</p>
          ) : (
            list.map((p) => (
              <button
                key={p.id}
                onClick={() => addTo(p.id)}
                disabled={busy}
                className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-2 text-left transition hover:bg-[var(--track-bg)] disabled:opacity-50"
              >
                <div className="grid h-9 w-9 flex-none place-items-center rounded-lg accent-bg text-white">
                  <Icons.Playlist className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium">{p.name}</div>
                  <div className="text-xs text-[var(--text-dim)]">{p.trackCount} 曲</div>
                </div>
                <Icons.Plus className="h-4 w-4 flex-none text-[var(--text-dim)]" />
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
