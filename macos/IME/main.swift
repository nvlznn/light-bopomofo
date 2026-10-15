import Cocoa
import InputMethodKit

// The input method. `install` / `uninstall` arguments run the installer from
// scripts (development, CI); the settings app does the same for users.

switch CommandLine.arguments.dropFirst().first {
case "install":
    do {
        let installer = Installer()
        try installer.install()
        print("installed \(installer.sourceVersion) at \(installer.destination.path)")
        exit(0)
    } catch {
        print("install failed: \(error.localizedDescription)")
        exit(1)
    }
case "uninstall":
    do {
        try Installer().uninstall()
        print("uninstalled")
        exit(0)
    } catch {
        print("uninstall failed: \(error.localizedDescription)")
        exit(1)
    }
default:
    break
}

guard Installer.isInputMethodsCopy() else {
    print("This is the input method itself; open 輕注音 (Light Bopomofo.app) to install it.")
    exit(0)
}

let bundleID = Bundle.main.bundleIdentifier ?? "dev.noky.inputmethod.lightbopomofo"
let connectionName =
    Bundle.main.infoDictionary?["InputMethodConnectionName"] as? String
    ?? "dev.noky.inputmethod.lightbopomofo_Connection"
guard let server = IMKServer(name: connectionName, bundleIdentifier: bundleID) else {
    log.error("cannot start IMKServer \(connectionName, privacy: .public)")
    exit(1)
}
_ = SharedEngine.engine  // load the lexicon now, not on the first keystroke

// The settings app edited the history: forget what it removed right away.
let historyObserver = DistributedNotificationCenter.default().addObserver(
    forName: UserData.historyChanged, object: nil, queue: .main
) { _ in
    SharedEngine.engine?.reloadHistory()
    log.info("history reloaded")
}

log.info("started")
withExtendedLifetime((server, historyObserver)) {
    NSApplication.shared.run()
}
