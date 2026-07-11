/** Escapes Discord markdown special chars, including `[`/`]` — required so a
 *  title/artist containing brackets (e.g. "Song [Official Video]") can't break
 *  out of the `[label]` portion of a masked link (see mdLink below). */
export function escapeMd(text: string): string {
  return text.replace(/([*_~`|[\]])/g, '\\$1');
}

/**
 * Renders `text` as a Discord masked link to `url` (`[label](url)`), so track
 * titles/artist names are directly clickable through to their source instead
 * of requiring the separate "ソースを開く" button. `url` is always one of our
 * own resolvers' `sourceUrl` values, which is already host-validated (see
 * urlPatterns.ts's classifyInput) — never raw, unvalidated user input.
 * Still guards against a URL containing an unescaped `)` or a space, either of
 * which would prematurely close/break the `(url)` portion of the markdown and
 * corrupt the rendered message — falls back to plain (unlinked) text instead.
 */
export function mdLink(text: string, url: string): string {
  const label = escapeMd(text);
  if (!label || /[)\s]/.test(url)) return label;
  return `[${label}](${url})`;
}
