import type { Feed } from "./feed.js";
import type { Strike } from "../types.js";

interface Cell {
  lat: number;
  lon: number;
  /** Velocity in degrees per tick. */
  dLat: number;
  dLon: number;
  /** Strikes per tick, roughly. */
  intensity: number;
}

/**
 * Synthetic feed: a handful of storm cells drift across the map and emit
 * clustered strikes. Deterministic enough to demo, lively enough to exercise
 * the map, the WebSocket fan-out, and the alert matcher end-to-end with no
 * external dependency or credentials.
 */
export class SimulatorFeed implements Feed {
  readonly name = "simulator";
  private timer?: ReturnType<typeof setInterval>;
  private seq = 0;
  private tick = 0;
  private cells: Cell[] = [];

  constructor(
    private readonly tickMs = 1000,
    /** Center the storms somewhere interesting; defaults to the US Midwest. */
    private readonly center: { lat: number; lon: number } = {
      lat: 39.5,
      lon: -98.35,
    },
  ) {}

  async start(onStrike: (s: Strike) => void): Promise<void> {
    this.cells = Array.from({ length: 3 }, (_, i) => ({
      lat: this.center.lat + (i - 1) * 2,
      lon: this.center.lon + (i - 1) * 3,
      dLat: 0.01 + i * 0.004,
      dLon: 0.03 - i * 0.006,
      intensity: 2 + i,
    }));

    this.timer = setInterval(() => {
      const now = Date.now();
      this.tick++;
      for (let c = 0; c < this.cells.length; c++) {
        const cell = this.cells[c]!;
        cell.lat += cell.dLat;
        cell.lon += cell.dLon;
        // Wrap cells back toward the center so they don't drift off forever.
        if (Math.abs(cell.lat - this.center.lat) > 6) cell.dLat *= -1;
        if (Math.abs(cell.lon - this.center.lon) > 9) cell.dLon *= -1;

        // Seed off the monotonic tick (not seq) so the count varies every tick
        // and can never get stuck at zero.
        const r = pseudoRandom(this.tick * 100 + c);
        const count = 1 + Math.floor(cell.intensity * r);
        for (let i = 0; i < count; i++) {
          const spread = 0.6;
          this.seq++;
          onStrike({
            id: `sim-${now}-${this.seq}`,
            lat: cell.lat + (pseudoRandom(this.seq) - 0.5) * spread,
            lon: cell.lon + (pseudoRandom(this.seq * 7 + 3) - 0.5) * spread,
            t: now,
            intensity: Math.round(pseudoRandom(this.seq * 13) * 80 + 5),
          });
        }
      }
    }, this.tickMs);
  }

  async stop(): Promise<void> {
    if (this.timer) clearInterval(this.timer);
    this.timer = undefined;
  }
}

/**
 * Deterministic pseudo-random in [0,1). Math.random() is intentionally avoided
 * so behavior is reproducible across runs.
 */
function pseudoRandom(seed: number): number {
  const x = Math.sin(seed * 12.9898 + 78.233) * 43758.5453;
  return x - Math.floor(x);
}
