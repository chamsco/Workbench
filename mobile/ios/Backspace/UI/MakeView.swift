import SwiftUI

/// Start something: a prompt box, templates that fill it, and the chats
/// you made earlier.
struct MakeView: View {
    @Environment(AppStore.self) private var store
    @State private var text = ""
    @FocusState private var focused: Bool

    struct Template: Identifiable {
        var title: String
        var line: String
        var prompt: String
        var id: String { title }
    }

    static let templates = [
        Template(title: "Daily todo", line: "Today's list from the open tickets",
                 prompt: "Make my todo list for today from the project's open tickets, most urgent first."),
        Template(title: "Review a diff", line: "Paste it, get the risky parts",
                 prompt: "Review this diff. Lead with anything that would break, then style:\n\n"),
        Template(title: "Explain an error", line: "The cause, then the fix",
                 prompt: "Explain this error and how to fix it:\n\n"),
        Template(title: "Plan a feature", line: "Tickets from one line",
                 prompt: "Turn this into a short plan with tickets I can approve:\n\n"),
    ]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                prompt
                Text("Templates").font(.title3.weight(.semibold)).padding(.top, 10)
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 12) {
                        ForEach(Self.templates) { t in
                            Button { text = t.prompt; focused = true } label: { TemplateCard(t: t) }
                                .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 16)
                }
                .padding(.horizontal, -16)
                if !store.chats.isEmpty {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text("Earlier").font(.title3.weight(.semibold))
                        Text("\(store.chats.count)").foregroundStyle(.secondary)
                    }
                    .padding(.top, 10)
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
                        ForEach(store.chats) { c in
                            Button { store.open(c.id) } label: { EarlierCard(chat: c) }
                                .buttonStyle(.plain)
                        }
                    }
                }
            }
            .padding(16)
            .padding(.bottom, 120)
        }
    }

    private var prompt: some View {
        VStack(alignment: .leading) {
            TextField("What should we make?", text: $text, axis: .vertical)
                .lineLimit(3...8)
                .focused($focused)
            Spacer(minLength: 12)
            HStack {
                Spacer()
                Button {
                    store.newChat(route: store.lastRoute, text: text)
                    text = ""
                } label: {
                    Image(systemName: "arrow.up")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(.white)
                        .frame(width: 38, height: 38)
                        .background(Circle().fill(Color.bsAccent))
                }
                .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityLabel("Start")
            }
        }
        .padding(18)
        .frame(minHeight: 190)
        .background(
            RoundedRectangle(cornerRadius: 26)
                .fill(LinearGradient(colors: [Color.bsAccent.opacity(0.22), Color.bsAccent.opacity(0.06)], startPoint: .topLeading, endPoint: .bottomTrailing))
        )
        .overlay(RoundedRectangle(cornerRadius: 26).stroke(Color.bsAccent.opacity(0.25)))
    }
}

struct TemplateCard: View {
    let t: MakeView.Template

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Image(systemName: "lightbulb").foregroundStyle(.secondary)
                Spacer()
                Image(systemName: "plus").font(.footnote).foregroundStyle(.secondary)
            }
            Spacer(minLength: 22)
            Text(t.title).font(.headline)
            Text(t.line).font(.footnote).foregroundStyle(.secondary).lineLimit(1)
        }
        .padding(16)
        .frame(width: 230, height: 128, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 22).fill(Color.bsCard))
        .overlay(RoundedRectangle(cornerRadius: 22).stroke(Color.bsEdge))
    }
}

struct EarlierCard: View {
    let chat: ChatSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(chat.title).font(.subheadline.weight(.semibold)).lineLimit(2, reservesSpace: true)
            HStack(spacing: 5) {
                Image(systemName: chat.route.kind == "local" ? "desktopcomputer" : "sparkle")
                Text(age(chat.updated) == "now" ? "now" : "\(age(chat.updated)) ago")
            }
            .font(.caption)
            .foregroundStyle(.secondary)
            Text(chat.preview)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(4, reservesSpace: true)
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .topLeading)
                .background(RoundedRectangle(cornerRadius: 12).fill(Color.bsCard2))
        }
        .padding(14)
        .background(RoundedRectangle(cornerRadius: 20).fill(Color.bsCard))
        .overlay(RoundedRectangle(cornerRadius: 20).stroke(Color.bsEdge))
    }
}
