import test from "node:test";
import assert from "node:assert/strict";
import { haversineKm, bearingDeg, compass, bboxContains } from "../geo.js";

test("haversine: known distance NYC -> LA is ~3936 km", () => {
  const d = haversineKm(40.7128, -74.006, 34.0522, -118.2437);
  assert.ok(Math.abs(d - 3936) < 30, `got ${d}`);
});

test("haversine: zero distance", () => {
  assert.equal(haversineKm(10, 20, 10, 20), 0);
});

test("bearing: due north and due east", () => {
  assert.ok(Math.abs(bearingDeg(0, 0, 1, 0) - 0) < 0.01);
  assert.ok(Math.abs(bearingDeg(0, 0, 0, 1) - 90) < 0.01);
});

test("compass: cardinal mapping", () => {
  assert.equal(compass(0), "N");
  assert.equal(compass(90), "E");
  assert.equal(compass(180), "S");
  assert.equal(compass(270), "W");
  assert.equal(compass(45), "NE");
});

test("bboxContains: inside and outside", () => {
  const box = { minLat: 0, minLon: 0, maxLat: 10, maxLon: 10 };
  assert.equal(bboxContains(box, 5, 5), true);
  assert.equal(bboxContains(box, 11, 5), false);
});
