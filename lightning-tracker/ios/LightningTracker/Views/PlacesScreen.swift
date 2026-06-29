import SwiftUI

struct PlacesScreen: View {
    @EnvironmentObject var state: AppState
    @EnvironmentObject var location: LocationManager
    @EnvironmentObject var push: PushManager
    @State private var editing: SavedPlace?
    @State private var showAdd = false

    var body: some View {
        NavigationStack {
            List {
                if !push.authorized {
                    Section {
                        Button {
                            push.requestAuthorization()
                        } label: {
                            Label("Enable lightning alerts", systemImage: "bell.badge")
                        }
                    } footer: {
                        Text("Allow notifications so we can warn you when lightning is near a saved place.")
                    }
                }

                Section("Saved places") {
                    if state.places.isEmpty {
                        Text("No places yet. Add home, work, or anywhere you care about.")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(state.places) { place in
                        Button { editing = place } label: { placeRow(place) }
                            .tint(.primary)
                    }
                    .onDelete { idx in state.places.remove(atOffsets: idx) }
                }
            }
            .navigationTitle("Alerts")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showAdd = true } label: { Image(systemName: "plus") }
                }
            }
            .sheet(item: $editing) { place in
                PlaceEditView(place: place) { updated in upsert(updated) }
            }
            .sheet(isPresented: $showAdd) {
                PlaceEditView(place: newPlace()) { created in upsert(created) }
            }
        }
    }

    private func placeRow(_ place: SavedPlace) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(place.name).font(.headline)
            Text("Within \(Int(place.radiusKm)) km · \(place.mode.label)")
                .font(.caption).foregroundStyle(.secondary)
        }
    }

    private func newPlace() -> SavedPlace {
        let loc = location.location?.coordinate
        return SavedPlace(
            name: "",
            lat: loc?.latitude ?? 39.5,
            lon: loc?.longitude ?? -98.35,
            radiusKm: 25,
            mode: .first
        )
    }

    private func upsert(_ place: SavedPlace) {
        if let i = state.places.firstIndex(where: { $0.id == place.id }) {
            state.places[i] = place
        } else {
            state.places.append(place)
        }
    }
}
