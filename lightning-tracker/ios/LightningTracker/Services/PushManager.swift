import Foundation
import UIKit
import UserNotifications

/// Handles notification permission and APNs registration. The resulting device
/// token is handed back via `onToken` so AppState can register it (along with
/// the user's saved places) with the backend.
final class PushManager: NSObject, ObservableObject, UNUserNotificationCenterDelegate {
    @Published var authorized = false
    var onToken: ((String) -> Void)?

    func configure() {
        UNUserNotificationCenter.current().delegate = self
    }

    func requestAuthorization() {
        UNUserNotificationCenter.current().requestAuthorization(
            options: [.alert, .sound, .badge]
        ) { granted, _ in
            DispatchQueue.main.async {
                self.authorized = granted
                if granted { UIApplication.shared.registerForRemoteNotifications() }
            }
        }
    }

    /// Called by the app delegate with the raw APNs token data.
    func didRegister(deviceToken: Data) {
        let hex = deviceToken.map { String(format: "%02x", $0) }.joined()
        onToken?(hex)
    }

    // Show banners even while the app is foregrounded.
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound, .list])
    }
}
