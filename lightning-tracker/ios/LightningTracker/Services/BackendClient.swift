import Foundation
import MapKit

/// REST client for the lightning-tracker backend.
struct BackendClient {
    var baseURL = AppConfig.backendBaseURL
    var session: URLSession = .shared

    /// Strikes inside the current map region (cold start + pan/zoom).
    func strikes(in region: MKCoordinateRegion, sinceMinutes: Int = 60) async throws -> [Strike] {
        let minLat = region.center.latitude - region.span.latitudeDelta / 2
        let maxLat = region.center.latitude + region.span.latitudeDelta / 2
        let minLon = region.center.longitude - region.span.longitudeDelta / 2
        let maxLon = region.center.longitude + region.span.longitudeDelta / 2

        var comps = URLComponents(url: baseURL.appendingPathComponent("strikes"),
                                  resolvingAgainstBaseURL: false)!
        comps.queryItems = [
            .init(name: "minLat", value: String(minLat)),
            .init(name: "minLon", value: String(minLon)),
            .init(name: "maxLat", value: String(maxLat)),
            .init(name: "maxLon", value: String(maxLon)),
            .init(name: "sinceMin", value: String(sinceMinutes)),
        ]
        let (data, _) = try await session.data(from: comps.url!)
        return try JSONDecoder().decode(StrikesResponse.self, from: data).strikes
    }

    /// Register / update this device and its alert geofences.
    func registerDevice(
        token: String,
        places: [SavedPlace],
        quietHours: QuietHours,
        muted: Bool
    ) async throws {
        struct Body: Encodable {
            let token: String
            let places: [SavedPlace]
            let muted: Bool
            let quietHours: QH?
            struct QH: Encodable { let start, end, utcOffsetMin: Int }
        }
        let qh: Body.QH? = quietHours.enabled
            ? .init(start: quietHours.start, end: quietHours.end, utcOffsetMin: quietHours.utcOffsetMin)
            : nil
        let body = Body(token: token, places: places, muted: muted, quietHours: qh)

        var req = URLRequest(url: baseURL.appendingPathComponent("devices"))
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(body)
        _ = try await session.data(for: req)
    }
}
