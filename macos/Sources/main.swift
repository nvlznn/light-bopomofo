import Carbon
import Cocoa
import InputMethodKit

let bundleID = Bundle.main.bundleIdentifier ?? "dev.noky.inputmethod.lightbopomofo"

/// `LightBopomofo install`: register with Text Input Sources and enable it,
/// so it shows up without a trip to System Settings.
func install() -> Int32 {
    let status = TISRegisterInputSource(Bundle.main.bundleURL as CFURL)
    guard status == noErr else {
        print("TISRegisterInputSource failed: \(status)")
        return 1
    }
    let filter = [kTISPropertyBundleID as String: bundleID] as CFDictionary
    guard let sources = TISCreateInputSourceList(filter, true)?.takeRetainedValue() as? [TISInputSource],
          !sources.isEmpty
    else {
        print("registered, but no input source found for \(bundleID)")
        return 1
    }
    for source in sources {
        let enabled = TISEnableInputSource(source)
        if enabled != noErr { print("TISEnableInputSource failed: \(enabled)") }
    }
    print("installed and enabled \(bundleID)")
    return 0
}

if CommandLine.arguments.dropFirst().first == "install" {
    exit(install())
}

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
