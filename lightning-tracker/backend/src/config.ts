export interface ApnsConfig {
  keyPath?: string;
  keyId?: string;
  teamId?: string;
  topic?: string;
  production: boolean;
}

export interface Config {
  port: number;
  feed: "simulator" | "blitzortung";
  strikeTtlMs: number;
  alertCooldownMs: number;
  stormWindowMs: number;
  apns: ApnsConfig;
}

function num(value: string | undefined, fallback: number): number {
  const n = value === undefined ? NaN : Number(value);
  return Number.isFinite(n) ? n : fallback;
}

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  const feed = env.FEED === "blitzortung" ? "blitzortung" : "simulator";
  return {
    port: num(env.PORT, 8787),
    feed,
    strikeTtlMs: num(env.STRIKE_TTL_MIN, 60) * 60_000,
    alertCooldownMs: num(env.ALERT_COOLDOWN_SEC, 120) * 1_000,
    stormWindowMs: num(env.STORM_WINDOW_MIN, 30) * 60_000,
    apns: {
      keyPath: env.APNS_KEY_PATH || undefined,
      keyId: env.APNS_KEY_ID || undefined,
      teamId: env.APNS_TEAM_ID || undefined,
      topic: env.APNS_TOPIC || undefined,
      production: env.APNS_PRODUCTION === "true",
    },
  };
}

export function apnsConfigured(c: ApnsConfig): boolean {
  return Boolean(c.keyPath && c.keyId && c.teamId && c.topic);
}
