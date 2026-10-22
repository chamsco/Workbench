import Foundation
import Observation

/// The app's state: which Mac it talks to, what it last heard, and the
/// actions the views call. It polls; the server-sent events in the
/// companion plan (docs/companion.md, step 2) replace that later.
@MainActor @Observable
final class AppStore {
    enum Tab: String, CaseIterable {
        case chat, tasks, make

        var title: String { rawValue.capitalized }
        var icon: String {
            switch self {
            case .chat: "bubble.left"
            case .tasks: "checklist"
            case .make: "square.and.pencil"
            }
        }
    }

    private(set) var backend: (any Backend)?
    private(set) var pairing: Pairing?
    private(set) var demo = false

    var tab: Tab = .chat
    var drawer = false
    var showSettings = false
    var showNewChat = false

    private(set) var chats: [ChatSummary] = []
    private(set) var openChat: ChatThread?
    private(set) var openChatId: String?
    private(set) var routes: [RouteOption] = []
    private(set) var project: ProjectInfo?
    /// The last call that failed, shown as a banner until one succeeds.
    var error: String?
    /// Approvals put aside with Hide, for this session only.
    var hidden: Set<Int> = []

    @ObservationIgnored private var loop: Task<Void, Never>?

    init() {
        let defaults = UserDefaults.standard
        if defaults.bool(forKey: "demo") {
            useDemo()
        } else if let p = Keychain.load() {
            connect(p)
        }
        if let t = defaults.string(forKey: "startTab").flatMap(Tab.init(rawValue:)) { tab = t }
        drawer = defaults.bool(forKey: "startDrawer")
        if let id = defaults.string(forKey: "startChat") { openChatId = id }
    }

    var machineName: String { demo ? "Backspace" : pairing?.name ?? "Backspace" }
    var anyBusy: Bool { chats.contains { $0.busy } || (openChat?.isBusy ?? false) }
    var pendingApprovals: [Approval] {
        (project?.state?.approvals ?? []).filter { $0.isPending && !hidden.contains($0.id) }
    }

    /// The route the last New chat used, so a quick send goes to the same model.
    var lastRoute: Route? {
        get { UserDefaults.standard.data(forKey: "lastRoute").flatMap { try? JSONDecoder().decode(Route.self, from: $0) } }
        set { UserDefaults.standard.set(newValue.flatMap { try? JSONEncoder().encode($0) }, forKey: "lastRoute") }
    }

    // MARK: Connection

    func connect(_ p: Pairing) {
        pairing = p
        demo = false
        Keychain.save(p)
        start(Companion(pairing: p))
    }

    func useDemo() {
        demo = true
        start(DemoBackend())
    }

    func unpair() {
        loop?.cancel()
        Keychain.save(nil)
        backend = nil
        pairing = nil
        demo = false
        chats = []
        openChat = nil
        openChatId = nil
        routes = []
        project = nil
        error = nil
        showSettings = false
    }

    private func start(_ b: any Backend) {
        backend = b
        loop?.cancel()
        loop = Task { [weak self] in
            var tick = 0
            while !Task.isCancelled {
                guard let self else { return }
                await self.refresh(full: tick % 4 == 0)
                tick += 1
                // Quicker while a reply streams in.
                try? await Task.sleep(for: .milliseconds(self.openChat?.isBusy == true ? 900 : 2500))
            }
        }
    }

    func refresh(full: Bool = true) async {
        guard let b = backend else { return }
        do {
            if full {
                chats = try await b.chats()
                project = try await b.project()
                if routes.isEmpty { routes = try await b.routes() }
            }
            if let id = openChatId { openChat = try await b.chat(id) }
            error = nil
        } catch is CancellationError {
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func run(_ work: @escaping (any Backend) async throws -> Void) {
        guard let b = backend else { return }
        Task {
            do {
                try await work(b)
                await refresh()
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    // MARK: Chat

    func open(_ id: String?) {
        openChatId = id
        openChat = nil
        tab = .chat
        drawer = false
        Task { await refresh(full: false) }
    }

    /// Sends into the open chat, or starts one with the last route.
    func send(_ text: String) {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, let b = backend else { return }
        // Show it at once; the next poll brings the server's copy.
        let now = Int(Date().timeIntervalSince1970 * 1000)
        openChat?.messages.append(ChatMessage(id: "local-\(now)", role: "user", text: text, at: now, status: "sending"))
        let existing = openChatId
        let route = lastRoute
        Task {
            do {
                var id = existing
                if id == nil {
                    let t = try await b.newChat(route: route)
                    openChatId = t.id
                    openChat = t
                    id = t.id
                }
                try await b.send(id!, text: text)
                await refresh()
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    func newChat(route: Route?, text: String) {
        if let route { lastRoute = route }
        openChatId = nil
        openChat = nil
        tab = .chat
        drawer = false
        showNewChat = false
        if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            run { b in
                let t = try await b.newChat(route: route)
                await MainActor.run { self.openChatId = t.id; self.openChat = t }
            }
        } else {
            send(text)
        }
    }

    func stop() {
        guard let id = openChatId else { return }
        run { try await $0.stop(id) }
    }

    func react(_ msg: ChatMessage, _ emoji: String) {
        guard let id = openChatId else { return }
        run { try await $0.react(id, msg: msg.id, emoji: emoji) }
    }

    func remember(_ text: String) {
        run { try await $0.remember(text) }
    }

    // MARK: Tasks

    func approve(_ a: Approval) {
        hidden.insert(a.id)
        run { try await $0.approve(a.id) }
    }

    func reject(_ a: Approval, feedback: String) {
        hidden.insert(a.id)
        run { try await $0.reject(a.id, feedback: feedback) }
    }

    func ticketTitle(_ key: String) -> String? {
        project?.state?.tickets.first { $0.key == key }?.title
    }
}

/// "now", "19m", "1h", "1d": the desktop sidebar's ages.
func age(_ ms: Int, now: Date = Date()) -> String {
    let s = max(0, Int(now.timeIntervalSince1970) - ms / 1000)
    switch s {
    case ..<60: return "now"
    case ..<3600: return "\(s / 60)m"
    case ..<86400: return "\(s / 3600)h"
    default: return "\(s / 86400)d"
    }
}
