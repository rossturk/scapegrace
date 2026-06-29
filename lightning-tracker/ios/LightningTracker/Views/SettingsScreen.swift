import SwiftUI

struct SettingsScreen: View {
    @EnvironmentObject var state: AppState

    private let hours = Array(0...23)

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Toggle("Mute all alerts", isOn: $state.muted)
                } footer: {
                    Text("Temporarily stop all push notifications without deleting your places.")
                }

                Section("Quiet hours") {
                    Toggle("Enabled", isOn: $state.quietHours.enabled)
                    if state.quietHours.enabled {
                        Picker("From", selection: $state.quietHours.start) {
                            ForEach(hours, id: \.self) { Text(hourLabel($0)).tag($0) }
                        }
                        Picker("To", selection: $state.quietHours.end) {
                            ForEach(hours, id: \.self) { Text(hourLabel($0)).tag($0) }
                        }
                    }
                }

                Section("Data") {
                    LabeledContent("Lightning", value: "Blitzortung community network")
                    LabeledContent("Radar", value: "RainViewer")
                } footer: {
                    Text("Lightning data © Blitzortung.org contributors (non-commercial). Radar © RainViewer. Strikes are relayed through your own backend.")
                }

                Section {
                    LabeledContent("Version", value: "0.1.0")
                }
            }
            .navigationTitle("Settings")
        }
    }

    private func hourLabel(_ h: Int) -> String {
        String(format: "%02d:00", h)
    }
}
