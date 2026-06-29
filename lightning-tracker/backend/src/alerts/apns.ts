import http2 from "node:http2";
import { readFileSync } from "node:fs";
import jwt from "jsonwebtoken";
import type { ApnsConfig } from "../config.js";
import { apnsConfigured } from "../config.js";

export interface PushResult {
  ok: boolean;
  skipped?: boolean;
  status?: number;
  reason?: string;
}

/**
 * Minimal APNs HTTP/2 client using token-based (.p8) auth. If APNs isn't
 * configured the client cleanly no-ops, so the rest of the system runs and is
 * testable without Apple credentials.
 */
export class ApnsClient {
  private cachedToken?: { jwt: string; mintedAt: number };
  private readonly host: string;

  constructor(private readonly cfg: ApnsConfig) {
    this.host = cfg.production
      ? "https://api.push.apple.com"
      : "https://api.sandbox.push.apple.com";
  }

  get enabled(): boolean {
    return apnsConfigured(this.cfg);
  }

  async send(
    deviceToken: string,
    payload: { title: string; body: string; data?: Record<string, unknown> },
  ): Promise<PushResult> {
    if (!this.enabled) return { ok: false, skipped: true };

    const body = JSON.stringify({
      aps: {
        alert: { title: payload.title, body: payload.body },
        sound: "default",
        "interruption-level": "time-sensitive",
      },
      ...payload.data,
    });

    return new Promise<PushResult>((resolve) => {
      const client = http2.connect(this.host);
      client.on("error", (err) =>
        resolve({ ok: false, reason: String(err) }),
      );
      const req = client.request({
        ":method": "POST",
        ":path": `/3/device/${deviceToken}`,
        authorization: `bearer ${this.authToken()}`,
        "apns-topic": this.cfg.topic!,
        "apns-push-type": "alert",
        "apns-priority": "10",
        "content-type": "application/json",
      });

      let status = 0;
      let resBody = "";
      req.on("response", (headers) => {
        status = Number(headers[":status"] ?? 0);
      });
      req.on("data", (chunk) => (resBody += chunk));
      req.on("end", () => {
        client.close();
        if (status === 200) resolve({ ok: true, status });
        else
          resolve({
            ok: false,
            status,
            reason: resBody || `HTTP ${status}`,
          });
      });
      req.on("error", (err) => {
        client.close();
        resolve({ ok: false, reason: String(err) });
      });
      req.end(body);
    });
  }

  /** APNs JWTs are valid up to 60 min; refresh well inside that window. */
  private authToken(): string {
    const now = Date.now();
    if (this.cachedToken && now - this.cachedToken.mintedAt < 50 * 60_000) {
      return this.cachedToken.jwt;
    }
    const key = readFileSync(this.cfg.keyPath!, "utf8");
    const token = jwt.sign({}, key, {
      algorithm: "ES256",
      keyid: this.cfg.keyId!,
      issuer: this.cfg.teamId!,
      expiresIn: "55m",
    });
    this.cachedToken = { jwt: token, mintedAt: now };
    return token;
  }
}
