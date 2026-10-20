import SwiftUI
import UserNotifications

/// The window (started by Main).
struct LightBopomofoApp: App {
    @NSApplicationDelegateAdaptor private var delegate: AppDelegate
    @StateObject private var model = AppModel()
    @AppStorage(OnboardingView.doneKey) private var tutorialDone = false

    var body: some Scene {
        Window("輕注音", id: "main") {
            Group {
                if tutorialDone {
                    TabView {
                        GeneralView()
                            .tabItem { Label("一般", systemImage: "keyboard") }
                        HistoryView()
                            .tabItem { Label("選字紀錄", systemImage: "clock.arrow.circlepath") }
                    }
                    .padding(20)
                    .frame(minWidth: 620, minHeight: 460)
                } else {
                    OnboardingView { tutorialDone = true }
                }
            }
            .environmentObject(model)
            .task { model.setUp() }
            .onReceive(NotificationCenter.default.publisher(for: AppModel.updateRequested)) { _ in
                model.checkForUpdate(thenInstall: true)
            }
            .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
                // Pick up words typed meanwhile and changes made in System Settings.
                model.refreshStatus()
                model.reloadHistory()
            }
        }
        .windowResizability(.contentMinSize)
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate {
    func applicationWillFinishLaunching(_ notification: Notification) {
        // Before launching ends, to hear about the notification that launched us.
        UNUserNotificationCenter.current().delegate = self
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    /// lightbopomofo:update, from the input method's menu: update right away.
    func application(_ application: NSApplication, open urls: [URL]) {
        if urls.contains(PendingUpdate.url) { AppModel.requestUpdate() }
    }

    /// The update notification was chosen: update right away.
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        if response.notification.request.content.categoryIdentifier == Updater.category {
            DispatchQueue.main.async { AppModel.requestUpdate() }
        }
        completionHandler()
    }

    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner])
    }
}
