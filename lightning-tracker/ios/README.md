# Lightning Tracker — iOS app

SwiftUI + MapKit app: live strike map, RainViewer radar overlay with a time
slider, saved-place alerts, and APNs push registration.

## Requirements

- Xcode 15+ (iOS 16 deployment target)
- The [backend](../backend) running and reachable from the Simulator/device

## Open the project

This repo ships **source only** (no `.xcodeproj`, which is build-machine
specific). Generate one of two ways:

**A. XcodeGen (recommended)**

```bash
brew install xcodegen
cd ios
xcodegen generate        # creates LightningTracker.xcodeproj from project.yml
open LightningTracker.xcodeproj
```

**B. By hand**

1. Xcode → New → App → "LightningTracker", SwiftUI, iOS 16.
2. Delete the template `ContentView`/`App` files.
3. Drag the `LightningTracker/` folder in (Create groups).
4. Set the Info.plist to `Resources/Info.plist`, add the entitlements file, and
   enable the **Push Notifications** capability.

## Configure

Edit `LightningTracker/App/AppConfig.swift`:

```swift
static let backendBaseURL = URL(string: "http://localhost:8787")!
```

- **Simulator → backend on same Mac:** `http://localhost:8787` works.
- **Real device:** use your Mac's LAN IP (e.g. `http://192.168.1.20:8787`) or a
  deployed URL. iOS blocks plain HTTP by default — use HTTPS in production, or
  add an ATS exception for local testing.

## What works where

| Feature | Simulator | Device |
|---------|-----------|--------|
| Map + radar overlay | ✅ | ✅ |
| Live strikes (WebSocket) | ✅ | ✅ |
| Saved places & settings | ✅ | ✅ |
| **APNs push** | ❌ (no token) | ✅ (needs Apple Developer + `.p8`) |

Push notifications require a paid Apple Developer account, the Push Notifications
capability, and the backend configured with your APNs key. Everything else runs
on the free Simulator.

## Structure

```
LightningTracker/
├── App/        entry point, AppConfig, APNs app delegate
├── Models/     Strike, SavedPlace, RadarFrame
├── Services/   BackendClient (REST), StrikeSocket (WS), Location, Push, Radar
├── Store/      AppState (single source of truth)
├── Map/        MKMapView wrapper + RainViewer MKTileOverlay
├── Views/      Map, Alerts, place editor, Settings
└── Resources/  Info.plist, entitlements
```
