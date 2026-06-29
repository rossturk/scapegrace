import SwiftUI

struct RootView: View {
    var body: some View {
        TabView {
            MapScreen()
                .tabItem { Label("Map", systemImage: "map") }
            PlacesScreen()
                .tabItem { Label("Alerts", systemImage: "bell") }
            SettingsScreen()
                .tabItem { Label("Settings", systemImage: "gearshape") }
        }
        .tint(.yellow)
    }
}
