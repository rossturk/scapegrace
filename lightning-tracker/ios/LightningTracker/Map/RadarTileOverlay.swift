import MapKit

/// MKTileOverlay that renders one RainViewer radar frame.
///
/// Tile URL: `{host}{path}/{size}/{z}/{x}/{y}/{color}/{smooth}_{snow}.png`
/// RainViewer caps useful zoom at 7, so tiles are over-zoomed (stretched)
/// beyond that rather than fetched at higher z.
final class RadarTileOverlay: MKTileOverlay {
    private let host: String
    private let path: String
    private let size: Int
    private let color: Int
    private let options: String

    init(host: String, frame: RadarFrame, size: Int = 256, color: Int = 4, smooth: Bool = true, snow: Bool = true) {
        self.host = host
        self.path = frame.path
        self.size = size
        self.color = color
        self.options = "\(smooth ? 1 : 0)_\(snow ? 1 : 0)"
        super.init(urlTemplate: nil)
        self.tileSize = CGSize(width: size, height: size)
        self.canReplaceMapContent = false
        self.maximumZ = 7
    }

    override func url(forTilePath tilePath: MKTileOverlayPath) -> URL {
        let z = min(tilePath.z, maximumZ)
        let str = "\(host)\(path)/\(size)/\(z)/\(tilePath.x)/\(tilePath.y)/\(color)/\(options).png"
        return URL(string: str)!
    }
}
