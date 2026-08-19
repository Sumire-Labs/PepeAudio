export interface RuntimeConfig {
  readonly apiBaseUrl: string;
  readonly demoMode: boolean;
  readonly devUserId: string | null;
  readonly csrfToken: string | null;
  readonly bootstrapGuildId: string | null;
}

function optional(value: string | undefined): string | null {
  const normalized = value?.trim();
  return normalized ? normalized : null;
}

export const runtimeConfig: RuntimeConfig = {
  apiBaseUrl: import.meta.env.VITE_API_BASE_URL?.trim() || "/api/v1",
  demoMode: import.meta.env.VITE_PEPEAUDIO_DEMO_MODE === "true",
  devUserId: optional(import.meta.env.VITE_PEPEAUDIO_DEV_USER_ID),
  csrfToken: optional(import.meta.env.VITE_PEPEAUDIO_DEV_CSRF_TOKEN),
  bootstrapGuildId: optional(import.meta.env.VITE_PEPEAUDIO_GUILD_ID)
};
