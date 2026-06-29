# Lightning Tracker — iOS App Plan

A native iOS app that shows live lightning strikes on a map with a weather-radar
overlay and pushes a notification when strikes happen near the user.

> Scope note: this is a product/engineering plan, not part of the Scapegrace
> game. It lives on the `claude/lightning-ios-app-plan-*` branch as a planning
> deliverable.

---

## 1. Live lightning data source

I evaluated the realistic options for a feed of geolocated, near-real-time
strikes. They split cleanly into a free community network and several
commercial networks.

### Recommendation

| Tier | Source | Latency | Coverage | Cost | License fit |
|------|--------|---------|----------|------|-------------|
| **Primary (start here)** | **Blitzortung / LightningMaps** | seconds | Global (community sensors) | Free | Non-commercial only; **must run your own relay server** |
| Upgrade (commercial) | **OpenWeather Lightning API** | minutes | Global | Paid tier | Commercial OK, simple REST |
| Upgrade (pro-grade) | **Xweather Lightning** | near-real-time | Global, proprietary network | Paid | Commercial OK, **native push webhooks** |
| Enterprise | Vaisala / Earth Networks (ENTLN) / DTN / The Weather Co. | sub-second | Global, high accuracy | $$$ | Commercial, contract |

**Plan: build on Blitzortung for v1, design the ingestion layer behind an
interface so we can swap in (or fall back to) a commercial feed later.**

### Why Blitzortung first

- **It's a live, real-time, global feed** sourced from a volunteer sensor
  network — the same data behind `map.blitzortung.org` and `lightningmaps.org`.
- Delivered over a **WebSocket/MQTT** stream. Community relays (e.g. the
  Home Assistant integration) expose a public **MQTT broker with geohash-based
  topics**, so a client subscribes only to its region instead of the global
  firehose. Messages carry **lat, lon, timestamp** (and derived
  distance/azimuth from a reference point).
- Free, which de-risks v1.

### The catch (drives the architecture)

Blitzortung's **data usage policy requires third-party apps to run their own
server** — clients may not connect directly to Blitzortung's servers, and use
is **non-commercial**. This is non-negotiable and is the single biggest
architectural constraint: **we need a small backend** regardless of which feed
we use. That backend is also exactly where push-notification logic belongs (see
§3), so it pulls double duty.

### Commercial upgrade path

- **OpenWeather Lightning API** — near-real-time REST, query by lat/lon +
  radius, ~7-day window. Easiest commercial drop-in.
- **Xweather Lightning** — proprietary detection network with **webhooks**: you
  register an HTTPS endpoint and they *push* strikes to you. This maps perfectly
  onto our notification pipeline and would let us cut server-side polling.
- **Vaisala**, **Earth Networks ENTLN**, **DTN**, **The Weather Company (IBM)** —
  highest accuracy / lowest latency, sold via contract. Overkill until there's
  real usage.

### Radar overlay source (separate from strike data)

- **RainViewer Weather Maps API** — free worldwide radar tiles, **PNG**,
  past 2 h at 10-min intervals plus short-range nowcast.
  - Metadata JSON: `https://api.rainviewer.com/public/weather-maps.json`
    (lists available `past[]` and `nowcast[]` frames, each with a `path` +
    `time`).
  - Tile template:
    `https://tilecache.rainviewer.com{path}/{size}/{z}/{x}/{y}/{color}/{smooth}_{snow}.png`
    - `{size}` = `256` or `512`
    - `{z}/{x}/{y}` = standard slippy-map tile coords (**max zoom 7**)
    - `{color}` = color-scheme id, `{smooth}_{snow}` = options e.g. `1_1`
  - Free with attribution; great for v1. Commercial radar (Xweather Maps iOS
    SDK, MapTiler Weather) is the upgrade if we outgrow it.

---

## 2. Architecture

```
 ┌─────────────┐   MQTT/WS    ┌──────────────────────────┐   APNs    ┌──────────┐
 │ Blitzortung │ ───────────▶ │   Relay + Alert backend  │ ────────▶ │  APNs    │ ─▶ iPhone
 │  (or comm.  │   strikes    │  (ingest, geo-index,     │           │ (Apple)  │
 │   feed)     │              │   match users, push)     │           └──────────┘
 └─────────────┘              │                          │
                              │  - WebSocket → clients   │ ◀── live strikes ── iOS app
                              │  - REST: recent strikes  │ ──  region query ─▶
                              │  - Device registry +     │
                              │    geofence prefs (DB)   │
                              └──────────────────────────┘
        RainViewer tiles  ──────────────────────────────────────────▶ iOS app (map overlay)
```

**Why a backend is required (not optional):**
1. Blitzortung's policy forbids direct client connections.
2. Push notifications must originate server-side — only the server can hold the
   APNs auth key and decide "this strike is within X km of device Y" while the
   app is backgrounded or closed.
3. It's the seam that lets us swap data providers without shipping a new app.

**Backend responsibilities:**
- Maintain one upstream connection to the lightning feed (MQTT/WS or webhook).
- Normalize strikes to `{lat, lon, t, intensity?}` and write to a short-TTL,
  geo-indexed store (Redis geo / PostGIS).
- Fan strikes out to connected app clients over WebSocket (live map).
- Match each strike against registered device geofences → enqueue APNs pushes,
  with per-device rate limiting / debouncing.
- REST endpoint for "recent strikes in this bbox" (cold start, pan/zoom).

**Suggested stack:** Swift/Vapor or Node/TypeScript backend; Redis for geo +
dedupe + rate limiting; APNs token-based auth (`.p8` key). Single small VM or
container to start.

---

## 3. iOS app design

### Map + radar overlay

- **MapKit** (`MKMapView`) — native, free, no tile budget for the basemap.
- **Radar overlay:** add an `MKTileOverlay` subclass whose `url(forTilePath:)`
  builds the RainViewer tile URL from the current frame's `path`. Insert it
  **above the basemap, below annotations** so strikes draw on top.
- **Radar animation:** fetch `weather-maps.json`, step through `past[]` +
  `nowcast[]` frames by swapping the overlay's frame path on a timer; a
  play/pause + scrubber controls the loop. Cache tiles (URLCache) to keep it
  smooth.
- **Strikes:** render as annotations or, at city scale, a lightweight overlay
  (e.g. point layer) colored by **recency** — bright for fresh (<2 min), fading
  over ~30–60 min, then dropped. Live strikes arrive via the backend WebSocket;
  initial state via the REST bbox query on launch and on significant pan/zoom.
- **User location:** Core Location for "center on me" and the proximity ring.

### Notifications

- **Goal:** "Lightning detected within N km of you" while the app is closed.
- **Mechanism:** **remote push via APNs**, decided server-side (background
  apps can't reliably watch a live feed; local-only notifications would require
  the app to be running).
- **Setup / registration flow:**
  1. App requests notification permission + (when-in-use → optionally always)
     location permission.
  2. App registers with APNs, sends its **device token + chosen alert center
     (lat/lon) + radius + quiet hours** to the backend.
  3. Backend stores the geofence; on a matching strike it pushes a localized
     alert (`"⚡ Lightning 6 km NE — moving toward you"`).
- **Alerting policy (tunable, server-side):**
  - Radius presets (e.g. 10 / 25 / 50 km) + a "first strike of a storm" vs
    "every strike" mode.
  - **Debounce / rate-limit** so one storm doesn't fire 40 pushes — e.g. at most
    one alert per device per storm-cell per cooldown window; escalate only if a
    strike gets meaningfully closer.
  - Quiet hours and a global mute.
- **Location modes:** "alert at my current location" (needs background location
  / significant-change updates, more battery + an `Always` permission) vs
  "alert at saved places" (home/work pins, no background location needed).
  **Recommend shipping saved-places first** — simpler permissions, no battery
  cost, covers the main use case.

### Screens (v1)

1. **Map** — basemap + radar overlay + live strikes + "center on me", radar
   play/scrub, layer toggle.
2. **Alerts** — saved places, per-place radius + mode, quiet hours, master
   toggle.
3. **Strike detail / list** — recent strikes near a selected place (distance,
   time, bearing).
4. **Settings / About** — units, color scheme, **data attribution**
   (Blitzortung + RainViewer credits are required).

---

## 4. Milestones

- **M0 — Spike (1 wk):** Stand up the relay backend against Blitzortung MQTT;
  prove we can receive strikes and re-emit over WebSocket. Throwaway map to
  eyeball it.
- **M1 — Map + radar (1–2 wk):** MapKit + RainViewer `MKTileOverlay` with frame
  animation. Render live strikes from the backend WS + REST bbox cold-start.
- **M2 — Notifications (1–2 wk):** APNs token-auth, device/geofence registry,
  saved-places alerts with debounce + quiet hours.
- **M3 — Polish (1 wk):** Onboarding/permissions, recency fade, settings,
  attribution, offline/empty states, battery review.
- **M4 — Optional:** current-location background alerts; evaluate swapping in a
  commercial feed (OpenWeather/Xweather) for SLA + commercial licensing.

---

## 5. Costs, risks, and decisions

**Costs**
- Apple Developer Program: $99/yr.
- Backend: one small VM/container + Redis — low single-digit $/mo to start.
- Data: Blitzortung + RainViewer free for v1; commercial feed is the cost step
  if/when we need SLA or go commercial.

**Risks / watch-items**
- **Licensing is the gating decision.** Blitzortung is **non-commercial** and
  requires our own relay. If this ever ships as a paid/ad product, we must move
  to a commercial feed (OpenWeather/Xweather/Vaisala). The provider-swap seam in
  §2 exists precisely for this.
- **Notification spam** is the biggest UX risk — the debounce/rate-limit policy
  is a v1 requirement, not a nice-to-have.
- **Battery** — avoid background location in v1; prefer saved-places alerts.
- **RainViewer max zoom 7** — radar gets coarse when zoomed in tight; that's
  expected, set user expectations or upgrade radar provider later.
- **Coverage gaps** — Blitzortung accuracy/density varies by region; a
  commercial network is more uniform.

**Open questions for the user**
1. Commercial or personal/non-commercial app? (Decides whether Blitzortung is
   viable long-term.)
2. Alert on **current location** (background location, battery cost) or
   **saved places** only for v1?
3. Backend language preference — **Swift/Vapor** (one language end-to-end) or
   **Node/TypeScript**?

---

## Sources

- [OpenWeather Lightning API](https://openweathermap.org/api/lightning)
- [Xweather Lightning API](https://www.xweather.com/products/weather-api/lightning)
- [Vaisala Real-Time Lightning Data Feed](https://www.vaisala.com/en/products/systems/lightning/real-time-data-feed)
- [Earth Networks ENTLN feed example (engelsjk)](https://github.com/engelsjk/python-lightning-earthnetworks)
- [DTN Lightning API](https://api.weather.mg/api-detail-pages/lightning-parameter.html)
- [The Weather Company Lightning API](https://developer.weather.com/docs/openapi/lightning-3-0)
- [Blitzortung live map](https://map.blitzortung.org/) · [LightningMaps.org docs](https://www.lightningmaps.org/doc/intro)
- [Blitzortung MQTT relay (Home Assistant component)](https://github.com/mrk-its/homeassistant-blitzortung)
- [RainViewer Weather Maps API](https://www.rainviewer.com/api/weather-maps-api.html) · [RainViewer API example](https://github.com/rainviewer/rainviewer-api-example)
- [Xweather Weather Maps iOS SDK](https://www.xweather.com/docs/ios-sdk/weather-maps)
