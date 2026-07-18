/**
 * DEV-only visual preview of the full player UI with canned data. Loaded via a
 * dynamic import guarded by import.meta.env.DEV + `?demo`, so it never ships in a
 * production build. Not part of the real app flow.
 */
import { useState } from 'react';
import type { GuildSnapshot, GuildSummary, Me, QueueItemDTO } from './api.ts';
import type { GuildSession } from './useGuildSession.ts';
import { Ambient } from './Ambient.tsx';
import { Player } from './Player.tsx';
import { Queue } from './Queue.tsx';
import { Sidebar } from './App.tsx';

// Inline data-URI gradients (no network) so the demo renders instantly.
const PALETTES: Record<string, [string, string]> = {
  midnight: ['#5b2a86', '#f14b8b'],
  blind: ['#ff5f6d', '#ffc371'],
  crush: ['#0f2027', '#2c5364'],
  redbone: ['#c0392b', '#e67e22'],
  lucky: ['#1d976c', '#93f9b9'],
};
const art = (seed: string) => {
  const [a, b] = PALETTES[seed] ?? ['#3a3a4a', '#7a7a8a'];
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='500' height='500'><defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='${a}'/><stop offset='1' stop-color='${b}'/></linearGradient></defs><rect width='500' height='500' fill='url(%23g)'/></svg>`;
  return `data:image/svg+xml;utf8,${svg.replace(/#/g, '%23')}`;
};

function track(id: string, title: string, artist: string, seed: string, ms: number): QueueItemDTO {
  return { id, title, artist, durationMs: ms, thumbnailUrl: art(seed), sourceType: 'youtube', sourceUrl: `https://youtu.be/${id}`, requestedBy: '1', requesterName: 'DemoUser', requesterAvatarUrl: null };
}

const snapshot: GuildSnapshot = {
  guildId: 'demo',
  status: 'playing',
  current: track('a', 'Midnight City', 'M83', 'midnight', 244000),
  elapsedMs: 96000,
  queue: [track('b', 'Blinding Lights', 'The Weeknd', 'blind', 200000), track('c', 'Instant Crush', 'Daft Punk', 'crush', 337000), track('d', 'Redbone', 'Childish Gambino', 'redbone', 326000)],
  history: [track('e', 'Get Lucky', 'Daft Punk', 'lucky', 369000)],
  loopMode: 'off',
  shuffleEnabled: true,
  autoplay: true,
  volume: 70,
  hrirMode: 'on',
  aura360Mode: 'off',
  hrirProfile: 'Aura_Headphone_V2',
  auraPresets: [
    { id: 'Aura_Headphone_V2', label: 'Aura Headphone V2' },
    { id: 'Aura_Studio_V2', label: 'Aura Studio V2' },
  ],
  stay247: false,
  permissionMode: 'same-voice-channel',
  voiceChannelId: 'vc',
  lastError: null,
  auraEnabled: true,
  viewer: { canControl: true, denyReason: null, inBotVoiceChannel: true },
};

export function Demo() {
  const [snap, setSnap] = useState(snapshot);
  const session: GuildSession = {
    snapshot: snap,
    receivedAt: Date.now(),
    loading: false,
    connected: true,
    async sendCommand(command) {
      // Reflect a few toggles locally so the demo feels alive.
      if (command.type === 'togglePlayPause') setSnap((s) => ({ ...s, status: s.status === 'playing' ? 'paused' : 'playing' }));
      if (command.type === 'toggleShuffle') setSnap((s) => ({ ...s, shuffleEnabled: !s.shuffleEnabled }));
      if (command.type === 'setVolume') setSnap((s) => ({ ...s, volume: command.percent }));
      return { ok: true, snapshot: snap };
    },
    refresh() {},
  };

  const me: Me = { userId: '1', username: 'DemoUser', avatarUrl: null };
  const guilds: GuildSummary[] = [
    { guildId: 'demo', name: 'My Server', iconUrl: null, hasActiveSession: true, status: 'playing', currentTitle: 'Midnight City' },
    { guildId: 'g2', name: 'Gaming Hub', iconUrl: null, hasActiveSession: false, status: 'idle', currentTitle: null },
    { guildId: 'g3', name: 'Chill Lounge', iconUrl: null, hasActiveSession: true, status: 'paused', currentTitle: 'Redbone' },
  ];

  return (
    <>
      <Ambient url={snap.current?.thumbnailUrl ?? null} />
      <div className="flex h-full">
        <Sidebar
          me={me}
          guilds={guilds}
          selectedGuildId="demo"
          view="player"
          theme="dark"
          open
          onSelectGuild={() => {}}
          onView={() => {}}
          onCloseSidebar={() => {}}
          onCycleTheme={() => {}}
          onLogout={() => {}}
        />
        <div className="hidden md:block md:w-[4.5rem] md:flex-none" />
        <main className="min-w-0 flex-1 overflow-hidden">
          <div className="grid h-full gap-4 p-4 lg:grid-cols-[1fr_23rem]">
            <Player session={session} onSaveTrack={() => {}} />
            <div className="glass flex min-h-0 flex-col rounded-3xl p-4">
              <Queue session={session} onSaveTrack={() => {}} />
            </div>
          </div>
        </main>
      </div>
    </>
  );
}
