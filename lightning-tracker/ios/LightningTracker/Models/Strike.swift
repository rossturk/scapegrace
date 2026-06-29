import Foundation
import CoreLocation

/// A single lightning strike, matching the backend's JSON shape.
struct Strike: Codable, Identifiable, Equatable {
    let id: String
    let lat: Double
    let lon: Double
    /// Epoch milliseconds.
    let t: Double
    let intensity: Double?

    var coordinate: CLLocationCoordinate2D {
        CLLocationCoordinate2D(latitude: lat, longitude: lon)
    }

    var date: Date {
        Date(timeIntervalSince1970: t / 1000)
    }

    /// Age in seconds, used to fade markers from fresh (bright) to old (dim).
    func age(now: Date = Date()) -> TimeInterval {
        now.timeIntervalSince(date)
    }
}

/// Envelope for the `/strikes` REST response.
struct StrikesResponse: Codable {
    let strikes: [Strike]
}

/// Envelope for live WebSocket frames: `{ "type": "strike", "strike": {...} }`.
struct StrikeFrame: Codable {
    let type: String
    let strike: Strike?
}
