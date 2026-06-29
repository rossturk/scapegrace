import Foundation
import CoreLocation

enum AlertMode: String, Codable, CaseIterable, Identifiable {
    case first   // one alert per storm
    case every   // every strike, rate-limited

    var id: String { rawValue }
    var label: String {
        switch self {
        case .first: return "First strike of a storm"
        case .every: return "Every nearby strike"
        }
    }
}

/// A place the user wants lightning alerts for. Encodes to the backend's
/// geofence shape.
struct SavedPlace: Codable, Identifiable, Equatable {
    var id: String = UUID().uuidString
    var name: String
    var lat: Double
    var lon: Double
    var radiusKm: Double
    var mode: AlertMode

    var coordinate: CLLocationCoordinate2D {
        CLLocationCoordinate2D(latitude: lat, longitude: lon)
    }

    enum CodingKeys: String, CodingKey {
        case id, name, lat, lon, radiusKm, mode
    }
}

/// Quiet-hours window (whole hours), sent to the backend with the device's
/// current UTC offset so server-side matching can respect local time.
struct QuietHours: Codable, Equatable {
    var enabled: Bool = false
    var start: Int = 22
    var end: Int = 7

    var utcOffsetMin: Int {
        TimeZone.current.secondsFromGMT() / 60
    }
}
