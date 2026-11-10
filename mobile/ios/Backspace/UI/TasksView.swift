import SwiftUI

/// What waits on you in the project open on the Mac. Active: approvals
/// (swipe right to approve, left to send back). Up next: its open tickets.
struct TasksView: View {
    @Environment(AppStore.self) private var store
    @State private var lane = Lane.active
    @State private var query = ""
    @State private var searching = false
    @State private var rejecting: Approval?

    enum Lane: String, CaseIterable {
        case active = "Active", upNext = "Up next"
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 20) {
                ForEach(Lane.allCases, id: \.self) { l in
                    Button { lane = l } label: {
                        Text(l.rawValue)
                            .font(.title2.weight(.semibold))
                            .foregroundStyle(lane == l ? Color.primary : Color.secondary)
                    }
                    .buttonStyle(.plain)
                }
                Spacer()
                Button { withAnimation { searching.toggle(); if !searching { query = "" } } } label: {
                    Image(systemName: "magnifyingglass").font(.system(size: 18))
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Search")
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 8)
            if searching {
                TextField("Search", text: $query)
                    .padding(10)
                    .background(RoundedRectangle(cornerRadius: 12).fill(Color.bsCard))
                    .padding(.horizontal, 16)
            }
            content.frame(maxHeight: .infinity)
        }
        .sheet(item: $rejecting) { a in RejectSheet(approval: a) }
    }

    @ViewBuilder private var content: some View {
        if let p = store.project, !p.open {
            EmptyState(icon: "folder", title: "No project is open",
                       line: "Open one in Code on \(store.machineName) and what waits on you lands here.")
        } else if store.project == nil {
            ProgressView()
        } else if lane == .active {
            let list = store.pendingApprovals.filter { matches(title($0) + $0.deliverable.summary) }
            if list.isEmpty {
                EmptyState(icon: "checkmark.seal", title: "What can I take off your plate?",
                           line: "When an agent finishes a plan or a ticket, you approve it here.")
            } else {
                ScrollView {
                    VStack(spacing: 14) {
                        ForEach(list) { a in
                            ApprovalCard(approval: a, title: title(a), onReject: { rejecting = a })
                        }
                    }
                    .padding(16)
                    .padding(.bottom, 150)
                }
            }
        } else {
            let list = (store.project?.state?.tickets ?? [])
                .filter { $0.isOpen && matches($0.key + " " + $0.title) }
                .sorted { ($0.rank, $0.num) < ($1.rank, $1.num) }
            if list.isEmpty {
                EmptyState(icon: "tray", title: "Nothing up next", line: "The project's open tickets show here.")
            } else {
                ScrollView {
                    VStack(spacing: 8) {
                        ForEach(list) { TicketRow(ticket: $0) }
                    }
                    .padding(16)
                    .padding(.bottom, 150)
                }
            }
        }
    }

    private func matches(_ s: String) -> Bool {
        query.isEmpty || s.localizedCaseInsensitiveContains(query)
    }

    private func title(_ a: Approval) -> String {
        if a.kind == "plan" {
            return "Approve the plan: \(a.tickets.count) ticket\(a.tickets.count == 1 ? "" : "s")"
        }
        if let key = a.tickets.first {
            return store.ticketTitle(key).map { "\(key) · \($0)" } ?? "Review \(key)"
        }
        return "Review the work"
    }
}

/// An approval as a card you can swipe: right approves, left sends it back.
struct ApprovalCard: View {
    @Environment(AppStore.self) private var store
    let approval: Approval
    let title: String
    let onReject: () -> Void
    @State private var dx: CGFloat = 0

    private let threshold: CGFloat = 110

    var body: some View {
        ZStack {
            // What a swipe reveals underneath.
            RoundedRectangle(cornerRadius: 24)
                .fill(dx >= 0 ? Color.bsGreen : Color.bsRed)
                .opacity(min(1, abs(dx) / threshold))
                .overlay(alignment: dx >= 0 ? .leading : .trailing) {
                    VStack(spacing: 6) {
                        Image(systemName: dx >= 0 ? "play.fill" : "arrow.uturn.backward")
                            .font(.system(size: 22, weight: .semibold))
                        Text(dx >= 0 ? "Approve" : "Send back").font(.subheadline.weight(.semibold))
                    }
                    .foregroundStyle(.white)
                    .padding(.horizontal, 28)
                }
            card
                .offset(x: dx)
                .rotationEffect(.degrees(Double(dx) / 40), anchor: .bottom)
                .simultaneousGesture(
                    DragGesture(minimumDistance: 24)
                        .onChanged { v in
                            if abs(v.translation.width) > abs(v.translation.height) { dx = v.translation.width }
                        }
                        .onEnded { _ in
                            if dx > threshold {
                                withAnimation(.snappy) { dx = 600 }
                                store.approve(approval)
                            } else if dx < -threshold {
                                withAnimation(.snappy) { dx = 0 }
                                onReject()
                            } else {
                                withAnimation(.snappy) { dx = 0 }
                            }
                        }
                )
        }
    }

    private var card: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: approval.kind == "plan" ? "lightbulb" : "doc.text.magnifyingglass")
                    .font(.system(size: 20))
                    .foregroundStyle(.secondary)
                Text(title).font(.headline)
            }
            Text(markdown(approval.deliverable.summary))
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .lineLimit(6)
            if let stat = approval.deliverable.diffStat ?? (approval.deliverable.files.isEmpty ? nil : "\(approval.deliverable.files.count) files") {
                Text(stat).font(.caption.monospaced()).foregroundStyle(.secondary)
            }
            HStack(spacing: 10) {
                Button { withAnimation { _ = store.hidden.insert(approval.id) } } label: {
                    Text("Hide").frame(maxWidth: .infinity).padding(.vertical, 12)
                        .overlay(Capsule().stroke(Color.bsEdge, lineWidth: 1.5))
                }
                Button { store.approve(approval) } label: {
                    Text("Approve").fontWeight(.semibold).frame(maxWidth: .infinity).padding(.vertical, 12)
                        .foregroundStyle(Color.bsBg)
                        .background(Capsule().fill(Color.primary))
                }
            }
            .buttonStyle(.plain)
        }
        .padding(18)
        .background(RoundedRectangle(cornerRadius: 24).fill(Color.bsCard))
        .overlay(RoundedRectangle(cornerRadius: 24).stroke(Color.bsEdge))
    }
}

struct TicketRow: View {
    let ticket: Ticket

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(ticket.key).font(.caption.monospaced()).foregroundStyle(.secondary)
            Text(ticket.title).font(.body).lineLimit(2)
            Spacer(minLength: 8)
            Text(ticket.stateLabel)
                .font(.caption.weight(.medium))
                .padding(.horizontal, 8).padding(.vertical, 3)
                .background(Capsule().fill(ticket.state == "in_progress" ? Color.bsAccent.opacity(0.2) : Color.bsCard2))
        }
        .padding(14)
        .background(RoundedRectangle(cornerRadius: 16).fill(Color.bsCard))
    }
}

/// Send an approval back with what should change.
struct RejectSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let approval: Approval
    @State private var feedback = ""

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 12) {
                Text(approval.deliverable.summary).font(.subheadline).foregroundStyle(.secondary).lineLimit(4)
                TextField("What should change?", text: $feedback, axis: .vertical)
                    .lineLimit(4...10)
                    .padding(14)
                    .background(RoundedRectangle(cornerRadius: 16).fill(Color.bsCard))
                Spacer()
            }
            .padding(16)
            .background(Color.bsBg)
            .navigationTitle("Send back")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Send") { store.reject(approval, feedback: feedback); dismiss() }
                        .disabled(feedback.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}
