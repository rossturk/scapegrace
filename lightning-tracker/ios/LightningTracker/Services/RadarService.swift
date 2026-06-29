import Foundation

/// Fetches RainViewer radar frame metadata. Refresh periodically (frames update
/// roughly every 10 minutes).
struct RadarService {
    var session: URLSession = .shared

    func load() async throws -> RainViewerMaps {
        let (data, _) = try await session.data(from: AppConfig.rainviewerMapsURL)
        return try JSONDecoder().decode(RainViewerMaps.self, from: data)
    }
}
