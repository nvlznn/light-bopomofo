import SwiftUI

@main
struct LightBopomofoApp: App {
    @NSApplicationDelegateAdaptor private var delegate: AppDelegate
    @StateObject private var model = AppModel()

    var body: some Scene {
        Window("輕注音", id: "main") {
            TabView {
                GeneralView()
                    .tabItem { Label("一般", systemImage: "keyboard") }
                HistoryView()
                    .tabItem { Label("選字紀錄", systemImage: "clock.arrow.circlepath") }
            }
            .padding(20)
            .frame(minWidth: 620, minHeight: 460)
            .environmentObject(model)
            .task { model.setUp() }
            .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
                // Pick up words typed meanwhile and changes made in System Settings.
                model.refreshStatus()
                model.reloadHistory()
            }
        }
        .windowResizability(.contentMinSize)
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}
