import SwiftUI
import UIKit

@main
struct LightningTrackerApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @StateObject private var state = AppState()
    @StateObject private var location = LocationManager()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(state)
                .environmentObject(location)
                .environmentObject(appDelegate.push)
                .onAppear {
                    appDelegate.push.onToken = { token in
                        Task { @MainActor in state.setDeviceToken(token) }
                    }
                    state.start()
                }
        }
    }
}

/// Bridges UIKit app lifecycle events (APNs registration) into the app.
final class AppDelegate: NSObject, UIApplicationDelegate {
    let push = PushManager()

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        push.configure()
        return true
    }

    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        push.didRegister(deviceToken: deviceToken)
    }

    func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        // In Simulator there's no APNs token; that's expected.
        print("APNs registration failed: \(error.localizedDescription)")
    }
}
