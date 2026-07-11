import {
  ButtonBuilder,
  ButtonStyle,
  ContainerBuilder,
  SectionBuilder,
  SeparatorBuilder,
  TextDisplayBuilder,
  ThumbnailBuilder,
} from 'discord.js';
import type { GuildPlayer } from '../player/GuildPlayer.js';
import { renderProgressBar } from './progressBar.js';
import { formatDuration } from '../util/time.js';
import { escapeMd, mdLink } from './panelMarkdown.js';
import { loopLabel, sourceIcon, statusGlyph } from './panelLabels.js';

export interface PanelBuildOptions {
  sofalizerAvailable: boolean;
}

/** Adds the now-playing section to `container`: the empty-state message when
 * there's no current track, otherwise the track info (title/artist/requester/
 * source), the separator, the progress bar + status line (queue/loop/shuffle),
 * and the optional lastError warning line. */
export function addNowPlayingSection(container: ContainerBuilder, player: GuildPlayer, opts: PanelBuildOptions): void {
  const track = player.currentTrack;
  const elapsed = player.getElapsedMs();
  const duration = track?.durationMs ?? null;

  if (!track) {
    container.addTextDisplayComponents(
      new TextDisplayBuilder().setContent('**再生中の曲はありません**\n`/play` で曲を再生してください。'),
    );
  } else {
    const infoSection = new SectionBuilder().addTextDisplayComponents(
      new TextDisplayBuilder().setContent(
        `**${mdLink(track.title, track.sourceUrl)}**\n${mdLink(track.artist, track.sourceUrl)}`,
      ),
      new TextDisplayBuilder().setContent(
        `リクエスト: <@${track.requestedBy}> • ${sourceIcon(track.sourceType)} ${track.sourceType}`,
      ),
    );
    if (track.thumbnailUrl) {
      infoSection.setThumbnailAccessory(new ThumbnailBuilder().setURL(track.thumbnailUrl));
    } else {
      infoSection.setButtonAccessory(
        new ButtonBuilder().setStyle(ButtonStyle.Link).setLabel('ソースを開く').setURL(track.sourceUrl),
      );
    }
    container.addSectionComponents(infoSection);
    container.addSeparatorComponents(new SeparatorBuilder());

    const bar = renderProgressBar(elapsed, duration);
    const progressText = new TextDisplayBuilder().setContent(
      `\`${statusGlyph(player)}\` \`${bar}\` \`${formatDuration(elapsed)} / ${formatDuration(duration)}\``,
    );
    const statusLine = [
      `キュー: ${player.queue.length}`,
      `ループ: ${loopLabel(player.loopMode)}`,
      `シャッフル: ${player.shuffleEnabled ? 'オン' : 'オフ'}`,
    ].join(' • ');

    container.addTextDisplayComponents(progressText, new TextDisplayBuilder().setContent(statusLine));

    if (player.lastError) {
      container.addTextDisplayComponents(new TextDisplayBuilder().setContent(`⚠️ ${escapeMd(player.lastError)}`));
    }
  }
}

// Fixed HRIR processing (see config/hrirProfiles.ts) - no user-facing choice;
// this just credits whichever profile is ACTUALLY applied to the current
// resource (player.usingHrir), not merely configured (player.hrirProfile) -
// if the file went missing mid-session, resourceFactory falls back silently
// and this footer should not keep claiming it's active.
export function addHrirFooter(container: ContainerBuilder, player: GuildPlayer): void {
  if (player.currentTrack && player.usingHrir) {
    container.addTextDisplayComponents(new TextDisplayBuilder().setContent('-# Dolby Home Theater® v4 使用中'));
  }
}
