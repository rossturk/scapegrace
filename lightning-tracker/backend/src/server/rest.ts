import express, { type Express, type Request, type Response } from "express";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { StrikeStore } from "../store/strikeStore.js";
import type { DeviceStore } from "../store/deviceStore.js";
import type { Device, Geofence } from "../types.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

interface Deps {
  strikes: StrikeStore;
  devices: DeviceStore;
  feedName: string;
  apnsEnabled: boolean;
}

function parseGeofences(input: unknown): Geofence[] {
  if (!Array.isArray(input)) return [];
  const out: Geofence[] = [];
  for (const raw of input) {
    if (
      raw &&
      typeof raw.id === "string" &&
      typeof raw.name === "string" &&
      typeof raw.lat === "number" &&
      typeof raw.lon === "number" &&
      typeof raw.radiusKm === "number"
    ) {
      out.push({
        id: raw.id,
        name: raw.name,
        lat: raw.lat,
        lon: raw.lon,
        radiusKm: raw.radiusKm,
        mode: raw.mode === "every" ? "every" : "first",
      });
    }
  }
  return out;
}

export function buildApp(deps: Deps): Express {
  const app = express();
  app.use(express.json());

  app.get("/health", (_req, res) => {
    res.json({
      ok: true,
      feed: deps.feedName,
      apns: deps.apnsEnabled,
      strikes: deps.strikes.size,
      devices: deps.devices.all().length,
    });
  });

  // Strikes inside a bounding box, for cold start and pan/zoom.
  // GET /strikes?minLat=..&minLon=..&maxLat=..&maxLon=..&sinceMin=30
  app.get("/strikes", (req: Request, res: Response) => {
    const q = req.query;
    const box = {
      minLat: Number(q.minLat),
      minLon: Number(q.minLon),
      maxLat: Number(q.maxLat),
      maxLon: Number(q.maxLon),
    };
    if (Object.values(box).some((v) => !Number.isFinite(v))) {
      res.status(400).json({ error: "minLat,minLon,maxLat,maxLon required" });
      return;
    }
    const sinceMin = Number(q.sinceMin);
    const sinceMs = Number.isFinite(sinceMin) ? sinceMin * 60_000 : undefined;
    res.json({ strikes: deps.strikes.inBBox(box, sinceMs) });
  });

  // Register or update a device + its alert geofences.
  app.post("/devices", (req: Request, res: Response) => {
    const b = req.body ?? {};
    if (typeof b.token !== "string" || b.token.length < 8) {
      res.status(400).json({ error: "valid device token required" });
      return;
    }
    const device: Device = {
      token: b.token,
      platform: "ios",
      places: parseGeofences(b.places),
      quietHours:
        b.quietHours && typeof b.quietHours.start === "number"
          ? {
              start: b.quietHours.start,
              end: b.quietHours.end,
              utcOffsetMin: b.quietHours.utcOffsetMin ?? 0,
            }
          : undefined,
      muted: Boolean(b.muted),
      updatedAt: Date.now(),
    };
    res.json({ device: deps.devices.upsert(device) });
  });

  app.delete("/devices/:token", (req: Request, res: Response) => {
    const removed = deps.devices.remove(req.params.token ?? "");
    res.status(removed ? 200 : 404).json({ removed });
  });

  // Static demo page for eyeballing the live feed in a browser.
  app.use(
    "/",
    express.static(path.join(__dirname, "../../public"), { index: "demo.html" }),
  );

  return app;
}
