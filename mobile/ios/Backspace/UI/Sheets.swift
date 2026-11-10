import SwiftUI

/// Cancel / New chat / Create: pick who answers, and optionally say the first
/// thing.
struct NewChatSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var route: Route?
    @State private var text = ""

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 18) {
                    VStack(spacing: 8) {
                        Image(systemName: "bubble.left.and.text.bubble.right")
                            .font(.system(size: 44))
                            .foregroundStyle(Color.bsAccent)
                        Text(route.map(label) ?? "Your default model").font(.subheadline).foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 12)

                    Menu {
                        Button("Your default model") { route = nil }
                        ForEach(store.routes) { r in
                            if r.models.isEmpty {
                                Button(r.name) { route = r.route(model: nil) }
                            } else {
                                Menu(r.name) {
                                    Button("Its default") { route = r.route(model: nil) }
                                    ForEach(r.models, id: \.self) { m in
                                        Button(m) { route = r.route(model: m) }
                                    }
                                }
                            }
                        }
                    } label: {
                        HStack {
                            Text("Model").foregroundStyle(.primary)
                            Spacer()
                            Text(route.map(label) ?? "Default").foregroundStyle(.secondary)
                            Image(systemName: "chevron.up.chevron.down").font(.footnote).foregroundStyle(.secondary)
                        }
                        .padding(16)
                        .background(RoundedRectangle(cornerRadius: 16).fill(Color.bsCard))
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        Text("First message").font(.subheadline).foregroundStyle(.secondary).padding(.leading, 4)
                        TextField("What should it work on?", text: $text, axis: .vertical)
                            .lineLimit(4...10)
                            .padding(16)
                            .background(RoundedRectangle(cornerRadius: 16).fill(Color.bsCard))
                    }
                }
                .padding(16)
            }
            .background(Color.bsBg)
            .navigationTitle("New chat")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") { store.newChat(route: route, text: text) }.fontWeight(.semibold)
                }
            }
        }
        .onAppear { route = store.lastRoute }
    }

    private func label(_ r: Route) -> String {
        let name = store.routes.first { $0.id == r.provider }?.name ?? r.provider
        return r.model.map { "\(name) · \($0)" } ?? name
    }
}

/// The Mac this phone is paired with, and the way out.
struct SettingsSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section {
                    if store.demo {
                        LabeledContent("Mode", value: "Demo")
                    } else if let p = store.pairing {
                        LabeledContent("Mac", value: p.name)
                        LabeledContent("Address", value: p.url.absoluteString)
                    }
                    LabeledContent("Status", value: store.error == nil ? "Connected" : "Can't reach it")
                    if let project = store.project?.state?.name {
                        LabeledContent("Open project", value: project)
                    }
                } footer: {
                    if let e = store.error { Text(e) }
                }
                Section {
                    Button(store.demo ? "Leave the demo" : "Unpair this phone", role: .destructive) { store.unpair() }
                } footer: {
                    Text("To unpair every phone at once, use Unpair all phones in Settings → Phone on the Mac.")
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }
}
