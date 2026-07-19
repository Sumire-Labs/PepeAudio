/** Escapes `[`/`]` too, so a bracketed title can't break out of a masked link's
 *  `[label]` (see mdLink). */
export function escapeMd(text: string): string {
  return text.replace(/([*_~`|[\]])/g, '\\$1');
}

/** Renders `[label](url)`. `url` must be a host-validated `sourceUrl`, not raw
 *  user input. A `)` or space in `url` would break the `(url)` markdown, so
 *  falls back to plain text in that case. */
export function mdLink(text: string, url: string): string {
  const label = escapeMd(text);
  if (!label || /[)\s]/.test(url)) return label;
  return `[${label}](${url})`;
}
