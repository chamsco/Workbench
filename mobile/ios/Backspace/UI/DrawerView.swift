import SwiftUI

/// The chats on your Mac, newest first with pinned ones on top, and New chat.
struct DrawerView: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                RoundButton(system: "arrow.left", label: "Close") {
                    withAnimation(.snappy) { store.drawer = false }
                }
                Text(store.machineName).font(.title2.weight(.semibold)).lineLimit(1)
                Spacer()
                Menu {
                    Button { store.showSettings = true } label: { Label("Settings", systemImage: "gearshape") }
                    Button { Task { await store.refresh() } } label: { Label("Refresh", systemImage: "arrow.clockwise") }
                } label: {
                    Image(systemName: "ellipsis")
                        .font(.system(size: 18, weight: .medium))
                        .frame(width: 46, height: 46)
                        .background(Circle().fill(Color.bsCard2))
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 16)
            .padding(.top, 4)
            .padding(.bottom, 14)

            Divider().padding(.horizontal, 16)

            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    Button { store.showNewChat = true } label: {
                        Label("New chat", systemImage: "plus")
                            .font(.title3.weight(.medium))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 14).padding(.vertical, 12)
                    }
                    .buttonStyle(.plain)

                    let pinned = store.chats.filter(\.pinned)
                    if !pinned.isEmpty {
                        section("Pinned")
                        ForEach(pinned) { row($0) }
                    }
                    section("Chats")
                    ForEach(store.chats.filter { !$0.pinned }) { row($0) }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 10)
            }
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .background(Color.bsBg.ignoresSafeArea())
    }

    private func section(_ title: String) -> some View {
        Text(title)
            .font(.footnote.weight(.medium))
            .foregroundStyle(.secondary)
            .padding(.horizontal, 14)
            .padding(.top, 14).padding(.bottom, 4)
    }

    private func row(_ c: ChatSummary) -> some View {
        let selected = c.id == store.openChatId
        return Button { store.open(c.id) } label: {
            HStack(spacing: 10) {
                if c.busy {
                    ProgressView().controlSize(.mini)
                } else if c.unread {
                    Circle().fill(Color.bsAccent).frame(width: 7, height: 7)
                }
                Text(c.title).lineLimit(1)
                Spacer(minLength: 8)
                Text(age(c.updated)).font(.footnote).foregroundStyle(.secondary)
            }
            .padding(.horizontal, 14).padding(.vertical, 12)
            .background(RoundedRectangle(cornerRadius: 16).fill(selected ? Color.bsCard2 : .clear))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
