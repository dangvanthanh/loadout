import SwiftUI

@main
struct SkillDockApp: App {
    @StateObject private var store = Store()

    var body: some Scene {
        MenuBarExtra {
            ContentView()
                .environmentObject(store)
        } label: {
            // Filled icon = updates waiting somewhere.
            Image(systemName: store.totalUpdates > 0 ? "square.stack.3d.up.fill" : "square.stack.3d.up")
        }
        .menuBarExtraStyle(.window)
    }
}
