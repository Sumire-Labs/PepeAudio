import { db } from './db.js';

export type PermissionMode = 'same-voice-channel' | 'dj-role' | 'requester-only';
export type AuraToggleSetting = 'off' | 'on';

export interface GuildSettings {
  guildId: string;
  defaultVolume: number;
  defaultHrirMode: AuraToggleSetting;
  /** The Aura 360° effect (widening + bass), independent of Aura HRIR (defaultHrirMode). */
  defaultAura360Mode: AuraToggleSetting;
  /**
   * HRIR profile id (filename without extension) from config/hrirProfiles.ts.
   * Persisted/round-tripped for schema compatibility, but GuildPlayer no
   * longer reads this — the profile is now fixed at construction to
   * getHrirProfiles()[0], not user-selectable per guild.
   */
  defaultHrirProfile: string | null;
  djRoleId: string | null;
  permissionMode: PermissionMode;
  stay247: boolean;
  /** Reserved for a future autoplay feature (queue empty -> auto-add related tracks) - not read anywhere yet; no command sets it. */
  autoplay: boolean;
  updatedAt: number;
}

interface GuildSettingsRow {
  guild_id: string;
  default_volume: number;
  // SQLite columns keep their legacy snake_case names for DB compatibility;
  // renaming them (to match the Aura HRIR / Aura 360° TS fields) would require a migration.
  default_spatial_mode: string;
  default_enhancer_mode: string;
  default_hrir_profile: string | null;
  dj_role_id: string | null;
  permission_mode: string;
  stay_247: number;
  autoplay: number;
  updated_at: number;
}

const DEFAULTS: Omit<GuildSettings, 'guildId' | 'updatedAt'> = {
  defaultVolume: 50, // see DEFAULT_VOLUME_PERCENT — GuildPlayer pins the starting volume regardless, this keeps the persisted default consistent
  defaultHrirMode: 'on', // Aura HRIR is on by default; users toggle it off via the panel button
  defaultAura360Mode: 'off', // Aura 360° is off by default; opt-in via its own panel button
  defaultHrirProfile: null,
  djRoleId: null,
  permissionMode: 'same-voice-channel',
  stay247: false,
  autoplay: false,
};

const selectStmt = db.prepare('SELECT * FROM guild_settings WHERE guild_id = ?');
// The default_spatial_mode / default_enhancer_mode columns keep their legacy
// snake_case names for DB compatibility (they now back the Aura HRIR / Aura 360°
// TS fields); only the bound @param names track the renamed fields.
const upsertStmt = db.prepare(`
  INSERT INTO guild_settings (guild_id, default_volume, default_spatial_mode, default_enhancer_mode, default_hrir_profile, dj_role_id, permission_mode, stay_247, autoplay, updated_at)
  VALUES (@guildId, @defaultVolume, @defaultHrirMode, @defaultAura360Mode, @defaultHrirProfile, @djRoleId, @permissionMode, @stay247, @autoplay, @updatedAt)
  ON CONFLICT(guild_id) DO UPDATE SET
    default_volume = excluded.default_volume,
    default_spatial_mode = excluded.default_spatial_mode,
    default_enhancer_mode = excluded.default_enhancer_mode,
    default_hrir_profile = excluded.default_hrir_profile,
    dj_role_id = excluded.dj_role_id,
    permission_mode = excluded.permission_mode,
    stay_247 = excluded.stay_247,
    autoplay = excluded.autoplay,
    updated_at = excluded.updated_at
`);

function rowToSettings(row: GuildSettingsRow): GuildSettings {
  return {
    guildId: row.guild_id,
    defaultVolume: row.default_volume,
    // Legacy snake_case columns feed the renamed Aura HRIR / Aura 360° TS fields.
    defaultHrirMode: row.default_spatial_mode === 'on' ? 'on' : 'off',
    defaultAura360Mode: row.default_enhancer_mode === 'on' ? 'on' : 'off',
    defaultHrirProfile: row.default_hrir_profile ?? null,
    djRoleId: row.dj_role_id,
    permissionMode: (['same-voice-channel', 'dj-role', 'requester-only'] as const).includes(
      row.permission_mode as PermissionMode,
    )
      ? (row.permission_mode as PermissionMode)
      : 'same-voice-channel',
    stay247: Boolean(row.stay_247),
    autoplay: Boolean(row.autoplay),
    updatedAt: row.updated_at,
  };
}

export function getGuildSettings(guildId: string): GuildSettings {
  const row = selectStmt.get(guildId) as GuildSettingsRow | undefined;
  if (!row) {
    return { guildId, ...DEFAULTS, updatedAt: 0 };
  }
  return rowToSettings(row);
}

export function upsertGuildSettings(
  guildId: string,
  partial: Partial<Omit<GuildSettings, 'guildId' | 'updatedAt'>>,
): GuildSettings {
  const current = getGuildSettings(guildId);
  const merged: GuildSettings = { ...current, ...partial, guildId, updatedAt: Date.now() };
  upsertStmt.run({
    guildId: merged.guildId,
    defaultVolume: merged.defaultVolume,
    defaultHrirMode: merged.defaultHrirMode,
    defaultAura360Mode: merged.defaultAura360Mode,
    defaultHrirProfile: merged.defaultHrirProfile,
    djRoleId: merged.djRoleId,
    permissionMode: merged.permissionMode,
    stay247: merged.stay247 ? 1 : 0,
    autoplay: merged.autoplay ? 1 : 0,
    updatedAt: merged.updatedAt,
  });
  return merged;
}
