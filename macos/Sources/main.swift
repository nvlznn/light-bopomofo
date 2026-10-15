import Cocoa
import InputMethodKit

// One binary, three roles:
//   LightBopomofo install | uninstall   scripted (development, CI)
//   opened from anywhere else            installer window (the disk image)
//   running from an Input Methods folder the input method itself

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

if !Installer.isInputMethodsCopy() {
    InstallerUI.run()
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
log.info("started")
withExtendedLifetime(server) {
    NSApplication.shared.run()
}
