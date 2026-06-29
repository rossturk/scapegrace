import SwiftUI
import MapKit

struct PlaceEditView: View {
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject var location: LocationManager
    @State var place: SavedPlace
    var onSave: (SavedPlace) -> Void

    private let radii: [Double] = [10, 25, 50]

    var body: some View {
        NavigationStack {
            Form {
                Section("Name") {
                    TextField("Home, Work, Cabin…", text: $place.name)
                }

                Section("Alert radius") {
                    Picker("Radius", selection: $place.radiusKm) {
                        ForEach(radii, id: \.self) { Text("\(Int($0)) km").tag($0) }
                    }
                    .pickerStyle(.segmented)
                }

                Section("When to alert") {
                    Picker("Mode", selection: $place.mode) {
                        ForEach(AlertMode.allCases) { Text($0.label).tag($0) }
                    }
                }

                Section("Location") {
                    Text(String(format: "%.4f, %.4f", place.lat, place.lon))
                        .font(.callout.monospaced())
                        .foregroundStyle(.secondary)
                    if location.location != nil {
                        Button("Use my current location") {
                            if let c = location.location?.coordinate {
                                place.lat = c.latitude
                                place.lon = c.longitude
                            }
                        }
                    }
                    Map(initialPosition: .region(MKCoordinateRegion(
                        center: place.coordinate,
                        span: MKCoordinateSpan(latitudeDelta: 0.5, longitudeDelta: 0.5)
                    )))
                    .frame(height: 160)
                    .clipShape(RoundedRectangle(cornerRadius: 10))
                    .allowsHitTesting(false)
                }
            }
            .navigationTitle(place.name.isEmpty ? "New place" : place.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { onSave(place); dismiss() }
                        .disabled(place.name.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
    }
}
