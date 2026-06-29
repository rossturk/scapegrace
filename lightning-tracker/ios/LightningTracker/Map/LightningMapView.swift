import SwiftUI
import MapKit

/// MapKit map with a RainViewer radar overlay, live strike annotations, and
/// circles for the user's saved places. Wraps MKMapView for SwiftUI.
struct LightningMapView: UIViewRepresentable {
    var strikes: [Strike]
    var places: [SavedPlace]
    var radarHost: String?
    var radarFrame: RadarFrame?
    /// Called when the user finishes panning/zooming, with the new region.
    var onRegionChange: (MKCoordinateRegion) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> MKMapView {
        let map = MKMapView()
        map.delegate = context.coordinator
        map.showsUserLocation = true
        map.region = MKCoordinateRegion(
            center: CLLocationCoordinate2D(latitude: 39.5, longitude: -98.35),
            span: MKCoordinateSpan(latitudeDelta: 12, longitudeDelta: 12)
        )
        return map
    }

    func updateUIView(_ map: MKMapView, context: Context) {
        context.coordinator.updateRadar(on: map, host: radarHost, frame: radarFrame)
        context.coordinator.updatePlaces(on: map, places: places)
        context.coordinator.updateStrikes(on: map, strikes: strikes)
    }

    final class Coordinator: NSObject, MKMapViewDelegate {
        private let parent: LightningMapView
        private var radarOverlay: RadarTileOverlay?
        private var currentFrameTime: Int?
        private var placeCircles: [MKCircle] = []
        private var shownStrikeIds = Set<String>()

        init(_ parent: LightningMapView) { self.parent = parent }

        // MARK: Radar overlay (kept beneath annotations)

        func updateRadar(on map: MKMapView, host: String?, frame: RadarFrame?) {
            guard let host, let frame else { return }
            if frame.time == currentFrameTime { return }
            currentFrameTime = frame.time
            if let old = radarOverlay { map.removeOverlay(old) }
            let overlay = RadarTileOverlay(host: host, frame: frame)
            radarOverlay = overlay
            map.insertOverlay(overlay, at: 0, level: .aboveRoads)
        }

        // MARK: Saved-place radius circles

        func updatePlaces(on map: MKMapView, places: [SavedPlace]) {
            map.removeOverlays(placeCircles)
            placeCircles = places.map {
                MKCircle(center: $0.coordinate, radius: $0.radiusKm * 1000)
            }
            map.addOverlays(placeCircles, level: .aboveRoads)
        }

        // MARK: Live strikes

        func updateStrikes(on map: MKMapView, strikes: [Strike]) {
            let incoming = Set(strikes.map(\.id))

            // Remove annotations that aged out of the buffer.
            let stale = map.annotations.compactMap { $0 as? StrikeAnnotation }
                .filter { !incoming.contains($0.strike.id) }
            map.removeAnnotations(stale)
            shownStrikeIds.subtract(stale.map { $0.strike.id })

            // Add only newly-seen strikes (cap to keep MapKit responsive).
            let fresh = strikes.suffix(800).filter { !shownStrikeIds.contains($0.id) }
            map.addAnnotations(fresh.map(StrikeAnnotation.init))
            shownStrikeIds.formUnion(fresh.map(\.id))
        }

        // MARK: Renderers

        func mapView(_ mapView: MKMapView, rendererFor overlay: MKOverlay) -> MKOverlayRenderer {
            if let tile = overlay as? MKTileOverlay {
                let r = MKTileOverlayRenderer(tileOverlay: tile)
                r.alpha = 0.6
                return r
            }
            if let circle = overlay as? MKCircle {
                let r = MKCircleRenderer(circle: circle)
                r.fillColor = UIColor.systemYellow.withAlphaComponent(0.08)
                r.strokeColor = UIColor.systemYellow.withAlphaComponent(0.6)
                r.lineWidth = 1.5
                return r
            }
            return MKOverlayRenderer(overlay: overlay)
        }

        func mapView(_ mapView: MKMapView, viewFor annotation: MKAnnotation) -> MKAnnotationView? {
            guard let strikeAnn = annotation as? StrikeAnnotation else { return nil }
            let id = "strike"
            let view = (mapView.dequeueReusableAnnotationView(withIdentifier: id) as? MKMarkerAnnotationView)
                ?? MKMarkerAnnotationView(annotation: annotation, reuseIdentifier: id)
            view.annotation = annotation
            view.glyphImage = UIImage(systemName: "bolt.fill")
            view.markerTintColor = strikeAnn.tintColor
            view.displayPriority = .defaultLow // let MapKit cluster/cull dense strikes
            return view
        }

        func mapView(_ mapView: MKMapView, regionDidChangeAnimated animated: Bool) {
            parent.onRegionChange(mapView.region)
        }
    }
}

/// MKAnnotation wrapper for a strike, tinted by recency.
final class StrikeAnnotation: NSObject, MKAnnotation {
    let strike: Strike
    init(_ strike: Strike) { self.strike = strike }

    var coordinate: CLLocationCoordinate2D { strike.coordinate }
    var title: String? { "Lightning" }

    /// Bright for fresh strikes, fading toward gray as they age (~30 min).
    var tintColor: UIColor {
        let age = strike.age()
        let f = max(0, min(1, 1 - age / (30 * 60)))
        return UIColor(hue: 0.13, saturation: 0.9, brightness: 0.6 + 0.4 * f, alpha: 0.5 + 0.5 * f)
    }
}
