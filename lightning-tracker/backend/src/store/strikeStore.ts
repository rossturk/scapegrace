import type { Strike } from "../types.js";
import { bboxContains, type BBox } from "../geo.js";

/**
 * In-memory ring buffer of recent strikes with TTL pruning.
 *
 * A linear scan is fine for the volumes a single region produces. If this ever
 * needs to serve the global firehose, swap the backing store for Redis GEO or
 * PostGIS behind this same interface — nothing else in the app touches strikes
 * directly.
 */
export class StrikeStore {
  private strikes: Strike[] = [];

  constructor(
    private readonly ttlMs = 60 * 60 * 1000,
    private readonly max = 50_000,
  ) {}

  add(s: Strike, now = Date.now()): void {
    this.strikes.push(s);
    this.prune(now);
  }

  prune(now = Date.now()): void {
    const cutoff = now - this.ttlMs;
    if (this.strikes.length && this.strikes[0]!.t < cutoff) {
      this.strikes = this.strikes.filter((s) => s.t >= cutoff);
    }
    if (this.strikes.length > this.max) {
      this.strikes = this.strikes.slice(this.strikes.length - this.max);
    }
  }

  /** Strikes inside a bounding box, optionally limited to the last `sinceMs`. */
  inBBox(box: BBox, sinceMs?: number, now = Date.now()): Strike[] {
    const cutoff = sinceMs === undefined ? -Infinity : now - sinceMs;
    return this.strikes.filter(
      (s) => s.t >= cutoff && bboxContains(box, s.lat, s.lon),
    );
  }

  recent(sinceMs: number, now = Date.now()): Strike[] {
    const cutoff = now - sinceMs;
    return this.strikes.filter((s) => s.t >= cutoff);
  }

  get size(): number {
    return this.strikes.length;
  }
}
