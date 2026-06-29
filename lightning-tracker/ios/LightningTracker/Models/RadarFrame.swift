import Foundation

/// One RainViewer radar frame. The tile URL is built from the API host plus
/// this frame's `path`:
/// `{host}{path}/{size}/{z}/{x}/{y}/{color}/{smooth}_{snow}.png`
struct RadarFrame: Codable, Identifiable, Equatable {
    let time: Int
    let path: String

    var id: Int { time }
    var date: Date { Date(timeIntervalSince1970: TimeInterval(time)) }
}

/// Decodes the subset of weather-maps.json we use.
struct RainViewerMaps: Codable {
    let host: String
    let radar: Radar

    struct Radar: Codable {
        let past: [RadarFrame]
        let nowcast: [RadarFrame]?
    }

    /// Past frames followed by nowcast, oldest -> newest, for the time slider.
    var orderedFrames: [RadarFrame] {
        radar.past + (radar.nowcast ?? [])
    }
}
