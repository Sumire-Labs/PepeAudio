import type { MediaProvider, PublicMediaPage } from "./types";

const MAX_PUBLIC_PAGE_URL_BYTES = 2_048;
const MAX_APPLE_SLUG_BYTES = 512;

export function parsePublicMediaPage(value: unknown): PublicMediaPage {
  const candidate = record(value);
  if (!mediaProvider(candidate.provider) ||
      !boundedUtf8Text(candidate.url, MAX_PUBLIC_PAGE_URL_BYTES)) invalid();
  let url: URL;
  try {
    url = new URL(candidate.url);
  } catch {
    return invalid();
  }
  if (url.protocol !== "https:" || url.port !== "" || url.username !== "" ||
      url.password !== "" || url.hash !== "" ||
      !providerPageMatches(candidate.provider, url)) invalid();
  return { provider: candidate.provider, url: url.toString() };
}

export function displayablePublicMediaPage(
  value: PublicMediaPage | null | undefined
): PublicMediaPage | null {
  if (value === null || value === undefined) return null;
  try {
    return parsePublicMediaPage(value);
  } catch {
    return null;
  }
}

function providerPageMatches(provider: MediaProvider, url: URL): boolean {
  if (provider === "spotify") {
    return url.hostname === "open.spotify.com" && url.search === "" &&
      /^\/track\/[A-Za-z0-9]{22}\/?$/u.test(url.pathname);
  }
  if (provider === "apple_music") return appleMusicPageMatches(url);
  if (provider === "youtube") {
    const watch = (url.hostname === "youtube.com" || url.hostname === "www.youtube.com") &&
      url.pathname === "/watch" && oneSearchParameter(url, "v", /^[A-Za-z0-9_-]{11}$/u);
    const short = url.hostname === "youtu.be" && url.search === "" &&
      /^\/[A-Za-z0-9_-]{11}\/?$/u.test(url.pathname);
    return watch || short;
  }
  return (url.hostname === "soundcloud.com" || url.hostname === "www.soundcloud.com" ||
      url.hostname === "m.soundcloud.com") && url.search === "" &&
    /^\/[^/]+\/[^/]+\/?$/u.test(url.pathname);
}

function appleMusicPageMatches(url: URL): boolean {
  const segments = canonicalPathSegments(url);
  if (url.hostname !== "music.apple.com" || segments === null || segments.length !== 4) {
    return false;
  }
  const [storefront, kind, slug, id] = segments as [string, string, string, string];
  if (!/^[a-z]{2}$/u.test(storefront) || !validAppleSlug(slug) ||
      !/^[0-9]{1,20}$/u.test(id)) return false;
  if (kind === "song") return url.search === "";
  return kind === "album" && oneSearchParameter(url, "i", /^[0-9]{1,20}$/u);
}

function canonicalPathSegments(url: URL): string[] | null {
  const segments = url.pathname.slice(1).split("/");
  if (segments.at(-1) === "") segments.pop();
  return segments.length > 0 && segments.every((segment) => segment.length > 0)
    ? segments
    : null;
}

function validAppleSlug(value: string): boolean {
  return value !== "." && value !== ".." && /^[\x21-\x7e]+$/u.test(value) &&
    new TextEncoder().encode(value).length <= MAX_APPLE_SLUG_BYTES;
}

function oneSearchParameter(url: URL, name: string, pattern: RegExp): boolean {
  return [...url.searchParams.keys()].length === 1 &&
    url.searchParams.getAll(name).length === 1 &&
    pattern.test(url.searchParams.get(name) ?? "");
}

function mediaProvider(value: unknown): value is MediaProvider {
  return value === "spotify" || value === "apple_music" || value === "youtube" ||
    value === "soundcloud";
}

function boundedUtf8Text(value: unknown, limit: number): value is string {
  return typeof value === "string" && value.length > 0 &&
    new TextEncoder().encode(value).length <= limit &&
    ![...value].some((character) => /\p{Cc}/u.test(character));
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) invalid();
  return value as Record<string, unknown>;
}

function invalid(): never {
  throw new Error("Public media page is invalid");
}
