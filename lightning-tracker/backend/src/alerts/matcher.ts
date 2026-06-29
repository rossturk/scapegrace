import type { AlertMatch, Device, Geofence, Strike } from "../types.js";
import { haversineKm, bearingDeg, compass } from "../geo.js";

/**
 * Decides which devices should be pushed for a given strike, and enforces the
 * anti-spam policy. This is the heart of the "don't fire 40 alerts per storm"
 * requirement, kept pure and clock-injectable so it can be unit-tested.
 */
export class AlertMatcher {
  /** key = `${token}|${placeId}` -> last alert timestamp (ms). */
  private lastAlert = new Map<string, number>();

  constructor(
    private readonly cooldownMs: number,
    private readonly stormWindowMs: number,
    private readonly now: () => number = () => Date.now(),
  ) {}

  match(strike: Strike, devices: Device[]): AlertMatch[] {
    const t = this.now();
    const out: AlertMatch[] = [];
    for (const device of devices) {
      if (device.muted) continue;
      if (this.inQuietHours(device, t)) continue;
      for (const place of device.places) {
        const distanceKm = haversineKm(
          place.lat,
          place.lon,
          strike.lat,
          strike.lon,
        );
        if (distanceKm > place.radiusKm) continue;
        if (!this.shouldFire(device.token, place, t)) continue;

        this.lastAlert.set(this.key(device.token, place.id), t);
        const bearing = bearingDeg(
          place.lat,
          place.lon,
          strike.lat,
          strike.lon,
        );
        out.push({
          device,
          place,
          strike,
          distanceKm,
          bearingDeg: bearing,
          compass: compass(bearing),
        });
      }
    }
    return out;
  }

  private shouldFire(token: string, place: Geofence, t: number): boolean {
    const last = this.lastAlert.get(this.key(token, place.id));
    if (last === undefined) return true;
    const gap = place.mode === "first" ? this.stormWindowMs : this.cooldownMs;
    return t - last >= gap;
  }

  private inQuietHours(device: Device, t: number): boolean {
    const q = device.quietHours;
    if (!q) return false;
    const offsetMin = q.utcOffsetMin ?? 0;
    const localHour = Math.floor(
      (((t / 3_600_000 + offsetMin / 60) % 24) + 24) % 24,
    );
    if (q.start === q.end) return false;
    if (q.start < q.end) return localHour >= q.start && localHour < q.end;
    // Wraps past midnight, e.g. 22 -> 7.
    return localHour >= q.start || localHour < q.end;
  }

  private key(token: string, placeId: string): string {
    return `${token}|${placeId}`;
  }
}

/** Human-readable push body, e.g. "⚡ Lightning 6 km NE of Home". */
export function alertBody(m: AlertMatch): string {
  const km = m.distanceKm < 10 ? m.distanceKm.toFixed(1) : Math.round(m.distanceKm).toString();
  return `⚡ Lightning ${km} km ${m.compass} of ${m.place.name}`;
}
