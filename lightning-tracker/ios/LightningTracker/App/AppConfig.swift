import Foundation

/// Central configuration. Point `backendBaseURL` at your relay/alert backend.
/// For the iOS Simulator talking to a backend on the same Mac, use
/// http://localhost:8787. For a device, use your Mac's LAN IP or a deployed URL.
enum AppConfig {
    /// Base URL of the lightning-tracker backend (no trailing slash).
    static let backendBaseURL = URL(string: "http://localhost:8787")!

    /// WebSocket URL derived from the base URL (http->ws, https->wss).
    static var webSocketURL: URL {
        var comps = URLComponents(url: backendBaseURL, resolvingAgainstBaseURL: false)!
        comps.scheme = comps.scheme == "https" ? "wss" : "ws"
        comps.path = "/ws"
        return comps.url!
    }

    /// RainViewer radar metadata (free, no key).
    static let rainviewerMapsURL = URL(string: "https://api.rainviewer.com/public/weather-maps.json")!
}
