import Foundation

/// A Mac that isn't there: canned chats, a project with approvals waiting,
/// and replies that stream after a beat. "Try a demo" on the pairing screen,
/// and `-demo YES` for the simulator screenshots in CI.
actor DemoBackend: Backend {
    private var threads: [ChatThread]
    private var approvals: [Approval]
    private let tickets: [Ticket]
    private var seq = 0

    init() {
        let now = Int(Date().timeIntervalSince1970 * 1000)
        let min = 60_000
        let claude = Route(kind: "cli", provider: "claude", model: nil)
        let local = Route(kind: "local", provider: "ollama", model: "qwen3:8b")
        func msg(_ id: String, _ role: String, _ text: String, _ ago: Int) -> ChatMessage {
            ChatMessage(id: id, role: role, text: text, at: now - ago * min, status: "done")
        }
        threads = [
            ChatThread(id: "t1", title: "Update flow that installs itself", updated: now - 19 * min, route: claude, messages: [
                msg("m1", "user", "Can the app install its own update instead of opening the DMG?", 25),
                msg("m2", "assistant", "Yes. It downloads the new build next to the running one, checks the signature, swaps the bundle on quit and relaunches. You get a toast with **Restart now** or **Later**.", 19),
            ]),
            ChatThread(id: "t2", title: "Shark ink on the apps page", updated: now - 60 * min, route: claude, messages: [
                msg("m3", "user", "Put the ASCII sharks behind the apps grid, white on light and black on dark.", 70),
                msg("m4", "assistant", "Done: the sharks swim at 8% opacity behind the grid and follow the theme. Settings → Appearance has a switch to turn them off.", 60),
            ]),
            ChatThread(id: "t3", title: "Rust ownership, slowly", updated: now - 1440 * min, route: local, messages: [
                msg("m5", "user", "Explain borrowing like I know Swift.", 1450),
                msg("m6", "assistant", "A `&T` is a read-only view you lend out; many can exist at once. A `&mut T` is exclusive: while it lives, nobody else can look. Swift copies on write to stay safe; Rust makes you say who holds what, and checks it when it compiles.", 1440),
            ]),
            ChatThread(id: "t4", title: "Weekend plan: learn Rust", updated: now - 4320 * min, route: local, messages: [
                msg("m7", "user", "Plan a weekend to learn Rust.", 4330),
                msg("m8", "assistant", "Saturday: the book's chapters 1–6 and rustlings. Sunday: build a small CLI that reads a file and counts words, then add tests.", 4320),
            ]),
        ]
        approvals = [
            Approval(id: 1, kind: "plan", agent: 0, deliverable: Deliverable(summary: "Split the iOS app into pairing, chat, tasks and make. Pairing reads the QR from Settings → Phone; tasks are the open project's approvals.", files: [], diffStat: nil), tickets: ["BS-41", "BS-42", "BS-43"], state: ApprovalState(state: "pending")),
            Approval(id: 2, kind: "deliverable", agent: 2, deliverable: Deliverable(summary: "The drawer lists chats newest first with pinned ones on top, and New chat opens a sheet with the model picker.", files: ["mobile/ios/Backspace/UI/DrawerView.swift", "mobile/ios/Backspace/UI/NewChatSheet.swift"], diffStat: "2 files changed, 184 insertions(+)"), tickets: ["BS-40"], state: ApprovalState(state: "pending")),
        ]
        tickets = [
            Ticket(num: 40, key: "BS-40", title: "Drawer with chats and New chat", state: "in_review", whatToBuild: "", assignee: 2),
            Ticket(num: 41, key: "BS-41", title: "Pair by QR code", state: "proposed", whatToBuild: "", assignee: nil),
            Ticket(num: 42, key: "BS-42", title: "Swipe right to approve", state: "proposed", whatToBuild: "", assignee: nil),
            Ticket(num: 43, key: "BS-43", title: "Push when a ticket waits on you", state: "proposed", whatToBuild: "", assignee: nil),
            Ticket(num: 39, key: "BS-39", title: "Serve local models to the phone", state: "in_progress", whatToBuild: "", assignee: 1),
        ]
    }

    private func next() -> String { seq += 1; return "d\(seq)" }
    private var nowMs: Int { Int(Date().timeIntervalSince1970 * 1000) }

    func chats() -> [ChatSummary] {
        threads.sorted { $0.updated > $1.updated }.map {
            ChatSummary(id: $0.id, title: $0.title, updated: $0.updated, pinned: false, route: $0.route,
                        preview: $0.messages.last?.text ?? "", busy: $0.isBusy, unread: false)
        }
    }

    func chat(_ id: String) throws -> ChatThread {
        guard let t = threads.first(where: { $0.id == id }) else { throw CompanionError(message: "no such chat") }
        return t
    }

    func newChat(route: Route?) -> ChatThread {
        let t = ChatThread(id: next(), title: "New chat", updated: nowMs, route: route ?? Route(kind: "cli", provider: "claude", model: nil), messages: [])
        threads.append(t)
        return t
    }

    func send(_ id: String, text: String) throws {
        guard let i = threads.firstIndex(where: { $0.id == id }) else { throw CompanionError(message: "no such chat") }
        let reply = next()
        threads[i].messages.append(ChatMessage(id: next(), role: "user", text: text, at: nowMs, status: "done"))
        threads[i].messages.append(ChatMessage(id: reply, role: "assistant", text: "", at: nowMs, status: "streaming"))
        if threads[i].title == "New chat" { threads[i].title = String(text.prefix(40)) }
        threads[i].updated = nowMs
        Task {
            try? await Task.sleep(for: .seconds(1.5))
            self.finish(id, reply, "This is the demo, so nobody is home. Pair with your Mac and the model you picked answers here.")
        }
    }

    private func finish(_ id: String, _ msg: String, _ text: String) {
        guard let i = threads.firstIndex(where: { $0.id == id }),
              let j = threads[i].messages.firstIndex(where: { $0.id == msg && $0.isLive }) else { return }
        threads[i].messages[j].text = text
        threads[i].messages[j].status = "done"
    }

    func react(_ id: String, msg: String, emoji: String) {
        guard let i = threads.firstIndex(where: { $0.id == id }),
              let j = threads[i].messages.firstIndex(where: { $0.id == msg }) else { return }
        // One tapback per message; the same one again removes it.
        threads[i].messages[j].reactions = threads[i].messages[j].reactions == [emoji] ? [] : [emoji]
    }

    func stop(_ id: String) {
        guard let i = threads.firstIndex(where: { $0.id == id }), let j = threads[i].messages.lastIndex(where: { $0.isLive }) else { return }
        threads[i].messages[j].status = "stopped"
    }

    func routes() -> [RouteOption] {
        [
            RouteOption(id: "claude", name: "Claude Code", kind: "cli", models: ["opus", "sonnet"]),
            RouteOption(id: "ollama", name: "Ollama", kind: "local", models: ["qwen3:8b", "llama3.2:3b"]),
        ]
    }

    func project() -> ProjectInfo {
        ProjectInfo(open: true, workspace: "~/code/workbench", state: ProjectState(name: "Workbench", approvals: approvals, tickets: tickets))
    }

    func approve(_ id: Int) { setState(id, ApprovalState(state: "approved")) }
    func reject(_ id: Int, feedback: String) { setState(id, ApprovalState(state: "rejected", feedback: feedback)) }
    func remember(_ text: String) {}

    private func setState(_ id: Int, _ state: ApprovalState) {
        if let i = approvals.firstIndex(where: { $0.id == id }) { approvals[i].state = state }
    }
}
