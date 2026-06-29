import { createServer } from "node:http";
import { loadConfig, apnsConfigured } from "./config.js";
import { StrikeStore } from "./store/strikeStore.js";
import { DeviceStore } from "./store/deviceStore.js";
import { SimulatorFeed } from "./feeds/simulator.js";
import { BlitzortungFeed } from "./feeds/blitzortung.js";
import type { Feed } from "./feeds/feed.js";
import { AlertMatcher, alertBody } from "./alerts/matcher.js";
import { ApnsClient } from "./alerts/apns.js";
import { buildApp } from "./server/rest.js";
import { StrikeSocket } from "./server/ws.js";

const cfg = loadConfig();

const strikes = new StrikeStore(cfg.strikeTtlMs);
const devices = new DeviceStore(process.env.DEVICE_DB || "devices.json");
const matcher = new AlertMatcher(cfg.alertCooldownMs, cfg.stormWindowMs);
const apns = new ApnsClient(cfg.apns);

const feed: Feed =
  cfg.feed === "blitzortung" ? new BlitzortungFeed() : new SimulatorFeed();

const app = buildApp({
  strikes,
  devices,
  feedName: feed.name,
  apnsEnabled: apnsConfigured(cfg.apns),
});
const server = createServer(app);
const sockets = new StrikeSocket(server);

feed.start((strike) => {
  strikes.add(strike);
  sockets.broadcast(strike);

  const matches = matcher.match(strike, devices.all());
  for (const m of matches) {
    apns
      .send(m.device.token, {
        title: m.place.name,
        body: alertBody(m),
        data: {
          place: m.place.id,
          lat: m.strike.lat,
          lon: m.strike.lon,
          distanceKm: m.distanceKm,
        },
      })
      .then((r) => {
        if (!r.ok && !r.skipped) {
          console.warn(`[apns] push failed: ${r.reason ?? r.status}`);
        }
      });
  }
});

server.listen(cfg.port, () => {
  console.log(
    `lightning-tracker backend on :${cfg.port}  feed=${feed.name}  ` +
      `apns=${apnsConfigured(cfg.apns) ? "on" : "off"}`,
  );
  console.log(`  demo:    http://localhost:${cfg.port}/`);
  console.log(`  health:  http://localhost:${cfg.port}/health`);
});

async function shutdown() {
  await feed.stop();
  server.close();
  process.exit(0);
}
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
