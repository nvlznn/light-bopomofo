import Foundation

/// The app's entry point: the window, or a job without one.
///
///   LightBopomofo --check-update [--now]   the background item (Updater)
///   LightBopomofo --install-update         update now, no window (tests)
@main
enum Main {
    static func main() {
        let arguments = CommandLine.arguments
        if let i = arguments.firstIndex(of: "--check-update") {
            let force = arguments.dropFirst(i + 1).first == "--now"
            Task {
                let code = await Updater.checkInBackground(force: force)
                print(["0": "up to date", "10": "update available", "20": "updated", "1": "check failed"]["\(code)"] ?? "")
                exit(code)
            }
            dispatchMain()
        }
        if arguments.contains("--install-update") {
            Task {
                do {
                    guard let version = try await UpdateCheck.available() else {
                        print("up to date (\(UpdateCheck.currentVersion))")
                        exit(0)
                    }
                    try await Updater.install(version)
                    print("updated to \(version)")
                    exit(0)
                } catch {
                    print("update failed: \(error.localizedDescription)")
                    exit(1)
                }
            }
            dispatchMain()
        }
        LightBopomofoApp.main()
    }
}
