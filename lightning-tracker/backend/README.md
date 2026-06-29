# Lightning Tracker — backend

Relay + alert service for the Lightning Tracker iOS app. Ingests live strikes,
fans them out to app clients over WebSocket, serves recent strikes over REST, and
pushes APNs alerts when a strike enters a registered device's geofence.

## Run

```bash
npm install
cp .env.example .env     # optional; sensible defaults work out of the box
npm start                # tsx src/index.ts
npm run dev              # watch mode
npm test                 # node:test unit tests
npm run typecheck        # tsc --noEmit
```

Open <http://localhost:8787/> for a live Leaflet demo (radar + streaming
strikes) to confirm the pipeline end to end.

## Configuration (env)

| Var | Default | Meaning |
|-----|---------|---------|
| `PORT` | `8787` | HTTP/WS port |
| `FEED` | `simulator` | `simulator` or `blitzortung` |
| `STRIKE_TTL_MIN` | `60` | How long strikes are retained/served |
| `ALERT_COOLDOWN_SEC` | `120` | Min gap between alerts per place in `every` mode |
| `STORM_WINDOW_MIN` | `30` | Silence window after first alert in `first` mode |
| `APNS_KEY_PATH` / `APNS_KEY_ID` / `APNS_TEAM_ID` / `APNS_TOPIC` | — | APNs token auth; pushes no-op until all are set |
| `APNS_PRODUCTION` | `false` | `true` => production APNs host |

## API

- `GET /health` — feed name, APNs on/off, counts.
- `GET /strikes?minLat&minLon&maxLat&maxLon&sinceMin` — strikes in a bbox.
- `POST /devices` — register/update a device:
  `{ token, places: [{id,name,lat,lon,radiusKm,mode}], quietHours?, muted? }`.
- `DELETE /devices/:token` — unregister.
- `WS /ws` — live strikes: `{type:"strike", strike}`. Send
  `{type:"subscribe", bbox:[minLat,minLon,maxLat,maxLon]}` to filter by viewport.

## Architecture

```
feed (simulator | blitzortung)
        │  onStrike
        ▼
   StrikeStore ── GET /strikes
        │
        ├── StrikeSocket ── WS /ws ──▶ app map
        └── AlertMatcher ──▶ ApnsClient ──▶ APNs ──▶ device
```

Each box is a small module behind an interface (`feeds/feed.ts`, the stores, the
matcher, the APNs client) so any one can be swapped — e.g. Redis GEO for the
store, or an Xweather webhook for the feed — without touching the rest.

## Notes on the feeds

- **simulator** (default): synthetic storm cells. Deterministic, dependency-free,
  great for development and the demo page.
- **blitzortung**: experimental adapter for the community WebSocket. The protocol
  is reverse-engineered and undocumented, so endpoints/format may change. Per
  Blitzortung's policy this relay must be self-hosted and used non-commercially.
  For a commercial/SLA deployment, implement a `Feed` against OpenWeather or
  Xweather instead.
