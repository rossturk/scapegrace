import test from "node:test";
import assert from "node:assert/strict";
import { AlertMatcher, alertBody } from "../alerts/matcher.js";
import type { Device, Strike } from "../types.js";

function device(overrides: Partial<Device> = {}): Device {
  return {
    token: "abcdef0123456789",
    platform: "ios",
    places: [
      { id: "home", name: "Home", lat: 40, lon: -75, radiusKm: 25, mode: "every" },
    ],
    updatedAt: 0,
    ...overrides,
  };
}

const near: Strike = { id: "s1", lat: 40.05, lon: -75.05, t: 0 };
const far: Strike = { id: "s2", lat: 10, lon: 10, t: 0 };

test("matches a strike inside the radius", () => {
  const m = new AlertMatcher(120_000, 1_800_000, () => 1000);
  const matches = m.match(near, [device()]);
  assert.equal(matches.length, 1);
  assert.equal(matches[0]!.place.id, "home");
  assert.ok(matches[0]!.distanceKm < 25);
});

test("ignores a strike outside the radius", () => {
  const m = new AlertMatcher(120_000, 1_800_000, () => 1000);
  assert.equal(m.match(far, [device()]).length, 0);
});

test("cooldown suppresses repeat alerts in 'every' mode", () => {
  let now = 1000;
  const m = new AlertMatcher(120_000, 1_800_000, () => now);
  assert.equal(m.match(near, [device()]).length, 1);
  now += 60_000; // < 120s cooldown
  assert.equal(m.match(near, [device()]).length, 0);
  now += 61_000; // now past cooldown
  assert.equal(m.match(near, [device()]).length, 1);
});

test("'first' mode stays silent for the whole storm window", () => {
  let now = 1000;
  const m = new AlertMatcher(120_000, 1_800_000, () => now);
  const d = device({
    places: [
      { id: "home", name: "Home", lat: 40, lon: -75, radiusKm: 25, mode: "first" },
    ],
  });
  assert.equal(m.match(near, [d]).length, 1);
  now += 1_700_000; // < 30 min storm window
  assert.equal(m.match(near, [d]).length, 0);
  now += 200_000; // past the window
  assert.equal(m.match(near, [d]).length, 1);
});

test("muted devices receive nothing", () => {
  const m = new AlertMatcher(120_000, 1_800_000, () => 1000);
  assert.equal(m.match(near, [device({ muted: true })]).length, 0);
});

test("quiet hours suppress alerts", () => {
  // 03:00 UTC; quiet 22:00 -> 07:00 wraps midnight and should suppress.
  const threeAm = 3 * 3_600_000;
  const m = new AlertMatcher(120_000, 1_800_000, () => threeAm);
  const d = device({ quietHours: { start: 22, end: 7, utcOffsetMin: 0 } });
  assert.equal(m.match(near, [d]).length, 0);
});

test("alertBody is human readable", () => {
  const m = new AlertMatcher(120_000, 1_800_000, () => 1000);
  const body = alertBody(m.match(near, [device()])[0]!);
  assert.match(body, /Lightning .* of Home/);
});
