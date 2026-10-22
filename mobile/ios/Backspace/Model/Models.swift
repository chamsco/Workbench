import Foundation

// The companion API's shapes (docs/companion.md, protocol 1). They mirror
// crates/core: chat::ThreadInfo, chat::Thread, chat::Msg, project::ProjectState.
// Keys arrive in snake_case; `Companion.decoder` converts them.

/// Who answers in a thread (core's `chat::Route`).
struct Route: Codable, Hashable, Sendable {
    var kind: String
    var provider: String
    var model: String?
}

/// A row of `GET /v1/chats`.
struct ChatSummary: Codable, Identifiable, Hashable, Sendable {
    var id: String
    var title: String
    var updated: Int
    var pinned: Bool
    var route: Route
    var preview: String
    var busy: Bool
    var unread: Bool
    var project: String?
    /// Why its last reply failed (a usage limit, a crash), if it did.
    var failed: String?
}

struct ChatMessage: Codable, Identifiable, Hashable, Sendable {
    var id: String
    var role: String
    var text: String
    var at: Int
    /// sending, streaming, done, error or stopped.
    var status: String
    var error: String?
    var replyTo: String?
    var reactions: [String]?
    var model: String?
    var authorName: String?

    var isUser: Bool { role == "user" }
    var isLive: Bool { status == "sending" || status == "streaming" }
}

/// `GET /v1/chat?id=`: one thread with its messages.
struct ChatThread: Codable, Identifiable, Hashable, Sendable {
    var id: String
    var title: String
    var updated: Int
    var route: Route
    var messages: [ChatMessage]

    var isBusy: Bool { messages.last?.isLive ?? false }
}

/// A row of `GET /v1/routes`: a harness switched on that can chat.
struct RouteOption: Codable, Identifiable, Hashable, Sendable {
    var id: String
    var name: String
    /// The harness kind: cli, local, router or agent.
    var kind: String
    var models: [String]

    /// The chat route for this harness. A connected agent chats over A2A.
    func route(model: String?) -> Route {
        Route(kind: kind == "agent" ? "a2a" : kind, provider: id, model: model)
    }
}

/// `GET /v1/project`: the project open on the Mac, if any.
struct ProjectInfo: Codable, Hashable, Sendable {
    var open: Bool
    var workspace: String?
    var state: ProjectState?
}

struct ProjectState: Codable, Hashable, Sendable {
    var name: String
    var approvals: [Approval]
    var tickets: [Ticket]
}

struct Approval: Codable, Identifiable, Hashable, Sendable {
    var id: Int
    /// "plan" (a batch of tickets before work starts) or "deliverable".
    var kind: String
    var agent: Int
    var deliverable: Deliverable
    /// The plan's tickets, or the one ticket delivered.
    var tickets: [String]
    var state: ApprovalState

    var isPending: Bool { state.state == "pending" }
}

/// Internally tagged in core: `{"state":"rejected","feedback":"..."}`.
struct ApprovalState: Codable, Hashable, Sendable {
    var state: String
    var feedback: String?
}

struct Deliverable: Codable, Hashable, Sendable {
    var summary: String
    var files: [String]
    var diffStat: String?
}

struct Ticket: Codable, Identifiable, Hashable, Sendable {
    var num: Int
    var key: String
    var title: String
    var state: String
    var whatToBuild: String
    var assignee: Int?

    var id: String { key }
    var isOpen: Bool { state != "done" && state != "wontfix" }
    var stateLabel: String { state.replacingOccurrences(of: "_", with: " ").capitalized }
    /// Work in motion first, then what waits on someone, then the rest.
    var rank: Int {
        ["in_progress", "in_review", "ready_for_human", "queued", "ready_for_agent", "needs_info", "needs_triage", "proposed", "failed"]
            .firstIndex(of: state) ?? 99
    }
}

/// `GET /v1/hello`, the one call without a token.
struct Hello: Codable, Sendable {
    var app: String
    var kind: String
    var `protocol`: Int
    var version: String
    var name: String
}
