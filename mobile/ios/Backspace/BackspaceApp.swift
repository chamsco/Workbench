import SwiftUI

@main
struct BackspaceApp: App {
    @State private var store = AppStore()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(store)
                .tint(Color.bsAccent)
                // backspace://pair?... from the camera app or a copied link.
                .onOpenURL { url in
                    guard let p = Pairing(link: url) else { return }
                    Task {
                        do {
                            _ = try await Companion.hello(at: p.url)
                            store.connect(p)
                        } catch {
                            store.error = error.localizedDescription
                        }
                    }
                }
                // Back from the background: catch up at once.
                .onChange(of: phase) {
                    if phase == .active { Task { await store.refresh() } }
                }
        }
    }
}
