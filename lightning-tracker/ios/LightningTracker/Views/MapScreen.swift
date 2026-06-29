import SwiftUI
import MapKit

struct MapScreen: View {
    @EnvironmentObject var state: AppState
    @EnvironmentObject var location: LocationManager

    var body: some View {
        ZStack(alignment: .top) {
            LightningMapView(
                strikes: state.strikes,
                places: state.places,
                radarHost: state.radar?.host,
                radarFrame: state.currentFrame,
                onRegionChange: { region in state.loadRecent(in: region) }
            )
            .ignoresSafeArea()

            statusBar
                .padding(.horizontal)
                .padding(.top, 8)

            VStack {
                Spacer()
                if let frames = state.radar?.orderedFrames, frames.count > 1 {
                    radarSlider(frames: frames)
                        .padding()
                }
            }
        }
        .onAppear { location.requestWhenInUse() }
    }

    private var statusBar: some View {
        HStack(spacing: 10) {
            Circle()
                .fill(state.connected ? Color.green : Color.orange)
                .frame(width: 8, height: 8)
            Text(state.connected ? "Live" : "Reconnecting…")
                .font(.caption.weight(.semibold))
            Spacer()
            Label("\(state.strikes.count)", systemImage: "bolt.fill")
                .font(.caption.weight(.semibold))
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.ultraThinMaterial, in: Capsule())
    }

    private func radarSlider(frames: [RadarFrame]) -> some View {
        VStack(spacing: 4) {
            if let frame = state.currentFrame {
                Text(frame.date.formatted(date: .omitted, time: .shortened))
                    .font(.caption2)
            }
            Slider(
                value: Binding(
                    get: { Double(state.selectedFrameIndex) },
                    set: { state.selectedFrameIndex = Int($0.rounded()) }
                ),
                in: 0...Double(frames.count - 1),
                step: 1
            )
        }
        .padding(10)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 12))
    }
}
