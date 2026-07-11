import type { GuildPlayer } from '../player/GuildPlayer.js';

export function sourceIcon(sourceType: string): string {
  switch (sourceType) {
    case 'youtube':
      return '▶️';
    case 'spotify':
      return '🟢';
    case 'soundcloud':
      return '🟠';
    case 'applemusic':
      return '🍎';
    default:
      return '🎵';
  }
}

export function loopLabel(mode: string): string {
  switch (mode) {
    case 'track':
      return '1曲';
    case 'queue':
      return '全体';
    default:
      return 'オフ';
  }
}

export function spatialLabel(player: GuildPlayer, sofalizerAvailable: boolean): string {
  if (player.spatialMode === 'off') return 'オフ';
  const engine = sofalizerAvailable ? 'バイノーラル' : '簡易';
  return `オン(${engine}・非推奨)`;
}

export function statusGlyph(player: GuildPlayer): string {
  if (!player.currentTrack) return '⏹';
  return player.isPaused() ? '⏸' : '▶';
}
