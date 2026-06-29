import test from "node:test";
import assert from "node:assert/strict";
import { StrikeStore } from "../store/strikeStore.js";
import type { Strike } from "../types.js";

const strike = (id: string, lat: number, lon: number, t: number): Strike => ({
  id,
  lat,
  lon,
  t,
});

test("prunes strikes older than the TTL", () => {
  const store = new StrikeStore(1000); // 1s TTL
  const now = 100_000;
  store.add(strike("old", 1, 1, now - 5000), now);
  store.add(strike("fresh", 1, 1, now - 100), now);
  assert.equal(store.size, 1);
  assert.equal(store.recent(2000, now)[0]!.id, "fresh");
});

test("inBBox filters by box and time window", () => {
  const store = new StrikeStore();
  const now = 100_000;
  store.add(strike("in", 5, 5, now - 1000), now);
  store.add(strike("out-box", 50, 50, now - 1000), now);
  store.add(strike("out-time", 5, 5, now - 999_999), now);
  const box = { minLat: 0, minLon: 0, maxLat: 10, maxLon: 10 };
  const got = store.inBBox(box, 60_000, now);
  assert.deepEqual(
    got.map((s) => s.id),
    ["in"],
  );
});
