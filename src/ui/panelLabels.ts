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

// Reflects the engine ACTUALLY applied to the current resource (player.usingHrir),
// not just whether the toggle is on: with a BRIR file present it's the real
// virtual-surround convolution, otherwise the asset-free wide fallback.
export function spatialLabel(player: GuildPlayer): string {
  if (player.spatialMode === 'off') return 'オフ';
  return player.usingHrir ? 'オン(バーチャルサラウンド)' : 'オン(ワイド)';
}

export function statusGlyph(player: GuildPlayer): string {
  if (!player.currentTrack) return '⏹';
  return player.isPaused() ? '⏸' : '▶';
}
