import type { Strike } from "../types.js";

/**
 * A source of live strikes. Implementations push normalized Strike objects to
 * the callback as they arrive. This is the seam that lets us swap Blitzortung
 * for a commercial provider (OpenWeather, Xweather webhooks, …) without
 * touching the rest of the app.
 */
export interface Feed {
  readonly name: string;
  start(onStrike: (s: Strike) => void): Promise<void>;
  stop(): Promise<void>;
}
