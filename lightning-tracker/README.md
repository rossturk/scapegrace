# Lightning Tracker

A native iOS app that shows **live lightning strikes** on a map with a **weather
radar overlay** and sends a **push notification** when lightning is near a place
you care about.

This is the implementation of [`lightning-ios-app-plan.md`](../lightning-ios-app-plan.md).

```
lightning-tracker/
├── backend/   Node + TypeScript relay + alert service (runnable, tested)
└── ios/       SwiftUI + MapKit app (open in Xcode)
```

## Why there's a backend

The lightning feed (Blitzortung) **requires self-hosting a relay** — clients may
not connect directly, and use is non-commercial. Push notifications also have to
be decided server-side so they work while the app is closed. The backend does
both: it ingests strikes, fans them out to the app over WebSocket, and matches
each strike against registered device geofences to send APNs pushes. A pluggable
feed interface lets a commercial provider (OpenWeather, Xweather webhooks, …)
drop in later without touching the app.

## Quick start

**1. Run the backend** (synthetic simulator feed — no credentials needed):

```bash
cd backend
npm install
npm start            # http://localhost:8787  (open / for a live demo map)
npm test             # unit tests for geo, store, and the alert matcher
```

**2. Run the app:** open `ios/` in Xcode (see [ios/README.md](ios/README.md)),
point `AppConfig.backendBaseURL` at your backend, and run on the Simulator. The
map fills with simulated strikes immediately; radar comes from RainViewer.

To use the **real** Blitzortung feed, start the backend with `FEED=blitzortung`
(experimental, see `backend/README.md`). To send **real pushes**, set the APNs
variables in `backend/.env` (you need an Apple Developer account + a `.p8` key).

## Data sources

- **Lightning:** [Blitzortung](https://www.blitzortung.org/) community network
  (non-commercial; self-hosted relay required).
- **Radar:** [RainViewer](https://www.rainviewer.com/api/weather-maps-api.html)
  free tiles.

See the [full plan](../lightning-ios-app-plan.md) for the commercial upgrade
path (OpenWeather / Xweather / Vaisala) and the rationale behind each choice.
