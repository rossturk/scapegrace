import Foundation

/// Live strike stream over a WebSocket, with automatic reconnect.
/// Delivers decoded strikes to `onStrike` on the main actor.
final class StrikeSocket: NSObject {
    private var task: URLSessionWebSocketTask?
    private lazy var session = URLSession(configuration: .default)
    private var isStopped = false

    var onStrike: ((Strike) -> Void)?
    var onStatusChange: ((Bool) -> Void)?

    func connect() {
        isStopped = false
        openSocket()
    }

    func stop() {
        isStopped = true
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        onStatusChange?(false)
    }

    private func openSocket() {
        let t = session.webSocketTask(with: AppConfig.webSocketURL)
        task = t
        t.resume()
        onStatusChange?(true)
        receive()
    }

    private func receive() {
        task?.receive { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let message):
                if case .string(let text) = message, let data = text.data(using: .utf8) {
                    if let frame = try? JSONDecoder().decode(StrikeFrame.self, from: data),
                       frame.type == "strike", let strike = frame.strike {
                        DispatchQueue.main.async { self.onStrike?(strike) }
                    }
                }
                self.receive() // keep listening
            case .failure:
                self.onStatusChange?(false)
                self.reconnectIfNeeded()
            }
        }
    }

    private func reconnectIfNeeded() {
        guard !isStopped else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in
            guard let self, !self.isStopped else { return }
            self.openSocket()
        }
    }
}
