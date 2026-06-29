import Foundation
import SwiftUI
import Combine
import MapKit

/// The app's single source of truth. Owns the live strike buffer, saved places,
/// alert preferences, radar frames, and the network services that feed them.
@MainActor
final class AppState: ObservableObject {
    // Live data
    @Published private(set) var strikes: [Strike] = []
    @Published private(set) var connected = false
    @Published private(set) var radar: RainViewerMaps?
    @Published var selectedFrameIndex: Int = 0

    // User config (persisted)
    @Published var places: [SavedPlace] = [] { didSet { persistPlaces(); syncDevice() } }
    @Published var quietHours = QuietHours() { didSet { persistConfig(); syncDevice() } }
    @Published var muted = false { didSet { persistConfig(); syncDevice() } }

    // Services
    private let backend = BackendClient()
    private let socket = StrikeSocket()
    private let radarService = RadarService()
    private var deviceToken: String?

    /// Cap the in-memory buffer so a long-running storm can't grow unbounded.
    private let maxStrikes = 2000
    /// Drop strikes older than this from the live map.
    private let strikeMaxAge: TimeInterval = 60 * 60

    init() {
        loadPersisted()
        socket.onStrike = { [weak self] in self?.add($0) }
        socket.onStatusChange = { [weak self] in self?.connected = $0 }
    }

    func start() {
        socket.connect()
        Task { await refreshRadar() }
    }

    // MARK: - Strikes

    private func add(_ strike: Strike) {
        strikes.append(strike)
        let cutoff = Date().addingTimeInterval(-strikeMaxAge)
        strikes.removeAll { $0.date < cutoff }
        if strikes.count > maxStrikes {
            strikes.removeFirst(strikes.count - maxStrikes)
        }
    }

    func loadRecent(in region: MKCoordinateRegion) {
        Task {
            if let fetched = try? await backend.strikes(in: region) {
                // Merge, de-duplicating by id.
                var byId = Dictionary(strikes.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
                for s in fetched { byId[s.id] = s }
                strikes = Array(byId.values).sorted { $0.t < $1.t }
            }
        }
    }

    // MARK: - Radar

    func refreshRadar() async {
        if let maps = try? await radarService.load() {
            radar = maps
            selectedFrameIndex = max(0, maps.orderedFrames.count - 1) // newest by default
        }
    }

    var currentFrame: RadarFrame? {
        guard let frames = radar?.orderedFrames, frames.indices.contains(selectedFrameIndex) else { return nil }
        return frames[selectedFrameIndex]
    }

    // MARK: - Device registration

    func setDeviceToken(_ token: String) {
        deviceToken = token
        syncDevice()
    }

    private func syncDevice() {
        guard let token = deviceToken else { return }
        let places = self.places, quietHours = self.quietHours, muted = self.muted
        Task {
            try? await backend.registerDevice(
                token: token, places: places, quietHours: quietHours, muted: muted
            )
        }
    }

    // MARK: - Persistence (UserDefaults)

    private func persistPlaces() {
        if let data = try? JSONEncoder().encode(places) {
            UserDefaults.standard.set(data, forKey: "places")
        }
    }

    private func persistConfig() {
        if let data = try? JSONEncoder().encode(quietHours) {
            UserDefaults.standard.set(data, forKey: "quietHours")
        }
        UserDefaults.standard.set(muted, forKey: "muted")
    }

    private func loadPersisted() {
        if let data = UserDefaults.standard.data(forKey: "places"),
           let decoded = try? JSONDecoder().decode([SavedPlace].self, from: data) {
            places = decoded
        }
        if let data = UserDefaults.standard.data(forKey: "quietHours"),
           let decoded = try? JSONDecoder().decode(QuietHours.self, from: data) {
            quietHours = decoded
        }
        muted = UserDefaults.standard.bool(forKey: "muted")
    }
}
