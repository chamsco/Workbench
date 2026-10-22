import Foundation
import Security

/// What the phone keeps after pairing: the Mac's address and its token.
struct Pairing: Codable, Equatable, Sendable {
    var url: URL
    var token: String
    var name: String
}

extension Pairing {
    /// The QR code's link: `backspace://pair?v=1&url=...&token=...&name=...`
    /// (core's `companion::pair_link`).
    init?(link: URL) {
        guard link.scheme == "backspace", link.host == "pair",
              let items = URLComponents(url: link, resolvingAgainstBaseURL: false)?.queryItems
        else { return nil }
        func value(_ key: String) -> String? { items.first { $0.name == key }?.value }
        guard let url = value("url").flatMap(URL.init(string:)),
              url.scheme == "http" || url.scheme == "https",
              let token = value("token"), !token.isEmpty
        else { return nil }
        self.init(url: url, token: token, name: value("name") ?? url.host() ?? "Backspace")
    }

    init?(link: String) {
        guard let url = URL(string: link.trimmingCharacters(in: .whitespacesAndNewlines)) else { return nil }
        self.init(link: url)
    }
}

struct CompanionError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

/// Everything the app asks of a Mac. `Companion` is the real one; `DemoBackend`
/// answers from memory so the app can be tried without pairing.
protocol Backend: Sendable {
    func chats() async throws -> [ChatSummary]
    func chat(_ id: String) async throws -> ChatThread
    func newChat(route: Route?) async throws -> ChatThread
    func send(_ id: String, text: String) async throws
    func react(_ id: String, msg: String, emoji: String) async throws
    func stop(_ id: String) async throws
    func routes() async throws -> [RouteOption]
    func project() async throws -> ProjectInfo
    func approve(_ id: Int) async throws
    func reject(_ id: Int, feedback: String) async throws
    func remember(_ text: String) async throws
}

/// The desktop app's phone API over HTTP (crates/core/src/companion.rs).
struct Companion: Backend {
    static let protocolVersion = 1
    let pairing: Pairing

    static var decoder: JSONDecoder {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        return d
    }

    /// Checks that `url` is a Backspace that speaks our protocol.
    static func hello(at url: URL) async throws -> Hello {
        var req = URLRequest(url: url.appending(path: "v1/hello"))
        req.timeoutInterval = 8
        let data: Data
        do {
            data = try await URLSession.shared.data(for: req).0
        } catch {
            throw CompanionError(message: "Can't reach \(url.host() ?? url.absoluteString). Is the phone on the same network, and is Let my phone connect on?")
        }
        guard let hello = try? decoder.decode(Hello.self, from: data), hello.kind == "companion" else {
            throw CompanionError(message: "That address answers, but it isn't Backspace.")
        }
        guard hello.protocol == protocolVersion else {
            throw CompanionError(message: "This Backspace speaks protocol \(hello.protocol); the app speaks \(protocolVersion). Update the older one.")
        }
        return hello
    }

    private struct ErrorBody: Decodable { var error: String }
    private struct Ignored: Decodable {}

    private func call(_ method: String, _ path: String, query: [URLQueryItem] = [], body: [String: Any]? = nil) async throws -> Data {
        var comps = URLComponents(url: pairing.url.appending(path: path), resolvingAgainstBaseURL: false)!
        if !query.isEmpty { comps.queryItems = query }
        var req = URLRequest(url: comps.url!)
        req.httpMethod = method
        req.timeoutInterval = 15
        req.setValue("Bearer \(pairing.token)", forHTTPHeaderField: "Authorization")
        if let body {
            req.httpBody = try JSONSerialization.data(withJSONObject: body)
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        let (data, resp) = try await URLSession.shared.data(for: req)
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(code) else {
            if code == 401 { throw CompanionError(message: "\(pairing.name) unpaired this phone. Scan its code again.") }
            let msg = (try? JSONDecoder().decode(ErrorBody.self, from: data))?.error
            throw CompanionError(message: msg ?? "\(pairing.name) answered \(code).")
        }
        return data
    }

    private func get<T: Decodable>(_ path: String, query: [URLQueryItem] = []) async throws -> T {
        try Self.decoder.decode(T.self, from: try await call("GET", path, query: query))
    }

    private func post(_ path: String, _ body: [String: Any]) async throws {
        _ = try await call("POST", path, body: body)
    }

    func chats() async throws -> [ChatSummary] { try await get("v1/chats") }
    func chat(_ id: String) async throws -> ChatThread { try await get("v1/chat", query: [URLQueryItem(name: "id", value: id)]) }
    func routes() async throws -> [RouteOption] { try await get("v1/routes") }
    func project() async throws -> ProjectInfo { try await get("v1/project") }

    func newChat(route: Route?) async throws -> ChatThread {
        var body: [String: Any] = [:]
        if let route {
            body["route"] = ["kind": route.kind, "provider": route.provider, "model": route.model.map { $0 as Any } ?? NSNull()] as [String: Any]
        }
        return try Self.decoder.decode(ChatThread.self, from: try await call("POST", "v1/chat/new", body: body))
    }

    func send(_ id: String, text: String) async throws { try await post("v1/chat/send", ["id": id, "text": text]) }
    func react(_ id: String, msg: String, emoji: String) async throws { try await post("v1/chat/react", ["id": id, "msg": msg, "emoji": emoji]) }
    func stop(_ id: String) async throws { try await post("v1/chat/stop", ["id": id]) }
    func approve(_ id: Int) async throws { try await post("v1/approve", ["id": id]) }
    func reject(_ id: Int, feedback: String) async throws { try await post("v1/reject", ["id": id, "feedback": feedback]) }
    func remember(_ text: String) async throws { try await post("v1/memory", ["text": text]) }
}

/// The pairing lives in the keychain, never in UserDefaults: the token is
/// the credential for every call.
enum Keychain {
    private static let base: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: "computer.backspace.phone",
        kSecAttrAccount as String: "pairing",
    ]

    static func load() -> Pairing? {
        var q = base
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return try? JSONDecoder().decode(Pairing.self, from: data)
    }

    static func save(_ pairing: Pairing?) {
        SecItemDelete(base as CFDictionary)
        guard let pairing, let data = try? JSONEncoder().encode(pairing) else { return }
        var q = base
        q[kSecValueData as String] = data
        q[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        SecItemAdd(q as CFDictionary, nil)
    }
}
