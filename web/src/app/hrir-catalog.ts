import type { HrirPreset } from "./types";

export function parseHrirPresetCatalog(
  value: unknown,
  expectedGuildId: string
): readonly HrirPreset[] {
  const catalog = record(value, "HRIR catalog");
  const guildId = snowflake(catalog.guild_id, "catalog guild_id");
  if (guildId !== expectedGuildId) throw new Error("HRIR catalog guild mismatch");
  if (!Array.isArray(catalog.presets) || catalog.presets.length > 1_000) {
    throw new Error("HRIR catalog presets are invalid");
  }

  const seen = new Set<string>();
  return catalog.presets.map((value, index) => {
    const preset = record(value, `HRIR preset ${index}`);
    const id = canonicalPresetId(preset.id, `HRIR preset ${index} id`);
    if (seen.has(id)) throw new Error("HRIR catalog contains duplicate IDs");
    seen.add(id);
    const source = record(preset.source, `HRIR preset ${index} source`);
    return {
      id,
      name: canonicalText(preset.display_name, `HRIR preset ${index} name`, 120),
      description: optionalText(
        preset.description,
        `HRIR preset ${index} description`,
        240
      ),
      source: {
        licenseName: optionalText(source.license_name, "license name", 256),
        sourceUrl: optionalHttpUrl(source.source_url),
        attribution: optionalText(source.attribution, "attribution", 4_096)
      }
    };
  });
}

function record(value: unknown, field: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${field} is invalid`);
  }
  return value as Record<string, unknown>;
}

function canonicalPresetId(value: unknown, field: string): string {
  const id = canonicalText(value, field, 128);
  if (new TextEncoder().encode(id).length > 128) throw new Error(`${field} is invalid`);
  return id;
}

function canonicalText(value: unknown, field: string, maxCharacters: number): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value) ||
    [...value].length > maxCharacters
  ) {
    throw new Error(`${field} is invalid`);
  }
  return value;
}

function optionalText(value: unknown, field: string, maxCharacters: number): string | null {
  if (value === undefined || value === null) return null;
  return canonicalText(value, field, maxCharacters);
}

function optionalHttpUrl(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  const text = canonicalText(value, "source URL", 2_048);
  let url: URL;
  try {
    url = new URL(text);
  } catch {
    throw new Error("source URL is invalid");
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error("source URL is invalid");
  }
  return text;
}

function snowflake(value: unknown, field: string): string {
  if (typeof value !== "string" || !/^[1-9][0-9]{0,19}$/u.test(value)) {
    throw new Error(`${field} is invalid`);
  }
  if (value.length === 20 && value > "18446744073709551615") {
    throw new Error(`${field} is invalid`);
  }
  return value;
}
