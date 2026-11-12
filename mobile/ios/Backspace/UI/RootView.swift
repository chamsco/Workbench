import SwiftUI

struct RootView: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        Group {
            if store.backend == nil {
                PairView()
            } else {
                MainView()
            }
        }
        .background(Color.bsBg.ignoresSafeArea())
    }
}

/// The phone's frame: menu, orb and machine across the top, the face of the
/// tab in the middle, and the floating Chat / Tasks / Make bar at the bottom.
/// The drawer slides in from the left and pushes the frame aside.
struct MainView: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        @Bindable var store = store
        GeometryReader { geo in
            let drawerWidth = min(geo.size.width * 0.84, 360)
            ZStack(alignment: .leading) {
                frame
                    .offset(x: store.drawer ? drawerWidth : 0)
                    .overlay {
                        if store.drawer {
                            Color.black.opacity(0.4)
                                .ignoresSafeArea()
                                .offset(x: drawerWidth)
                                .onTapGesture { withAnimation(.snappy) { store.drawer = false } }
                        }
                    }
                if store.drawer {
                    DrawerView()
                        .frame(width: drawerWidth)
                        .transition(.move(edge: .leading))
                }
            }
        }
        .sheet(isPresented: $store.showSettings) { SettingsSheet() }
        .sheet(isPresented: $store.showNewChat) { NewChatSheet() }
    }

    private var frame: some View {
        @Bindable var store = store
        return VStack(spacing: 0) {
            header
            if let error = store.error {
                Label(error, systemImage: "exclamationmark.triangle")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 14).fill(Color.bsCard))
                    .padding(.horizontal, 16).padding(.top, 6)
            }
            ZStack {
                switch store.tab {
                case .chat: ChatView()
                case .tasks: TasksView()
                case .make: MakeView()
                }
            }
            .frame(maxHeight: .infinity)
        }
        .overlay(alignment: .bottomTrailing) {
            if store.tab != .chat {
                Button { store.showNewChat = true } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 24, weight: .medium))
                        .foregroundStyle(.white)
                        .frame(width: 60, height: 60)
                        .background(Circle().fill(Color.bsAccent))
                        .shadow(color: .black.opacity(0.3), radius: 10, y: 4)
                }
                .accessibilityLabel("New chat")
                .padding(.trailing, 20).padding(.bottom, 84)
            }
        }
        .overlay(alignment: .bottom) {
            VStack(spacing: 10) {
                if store.tab == .chat { Composer() }
                TabBar(tab: $store.tab)
            }
            .padding(.bottom, 4)
        }
        .background(Color.bsBg.ignoresSafeArea())
    }

    private var header: some View {
        HStack {
            RoundButton(system: "line.3.horizontal", label: "Chats") {
                withAnimation(.snappy) { store.drawer = true }
            }
            Spacer()
            Orb(busy: store.anyBusy)
            Spacer()
            Button { store.showSettings = true } label: {
                Text(String(store.machineName.prefix(1)).uppercased())
                    .font(.title3.weight(.medium))
                    .frame(width: 46, height: 46)
                    .background(Circle().fill(Color.bsCard2))
            }
            .buttonStyle(.plain)
            .accessibilityLabel("\(store.machineName) settings")
        }
        .padding(.horizontal, 16)
        .padding(.top, 4)
        .padding(.bottom, 8)
    }
}

/// Chat / Tasks / Make, floating over the content.
struct TabBar: View {
    @Binding var tab: AppStore.Tab

    var body: some View {
        HStack(spacing: 2) {
            ForEach(AppStore.Tab.allCases, id: \.self) { t in
                Button { tab = t } label: {
                    VStack(spacing: 3) {
                        Image(systemName: t.icon).font(.system(size: 18))
                        Text(t.title).font(.caption.weight(.medium))
                    }
                    .frame(width: 94, height: 52)
                    .background(Capsule().fill(tab == t ? Color.bsCard2 : .clear))
                    .foregroundStyle(tab == t ? Color.primary : Color.secondary)
                    .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(tab == t ? .isSelected : [])
            }
        }
        .padding(5)
        .background(.ultraThinMaterial, in: Capsule())
        .overlay(Capsule().stroke(Color.bsEdge))
        .shadow(color: .black.opacity(0.25), radius: 14, y: 6)
    }
}
