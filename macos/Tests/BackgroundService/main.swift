import Foundation
import ServiceManagement

@main
struct Tests {
    @MainActor static func main() async throws {
        let suite = "dev.noky.tests.background.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        var state = SMAppService.Status.enabled
        var events: [String] = []
        var completion: CheckedContinuation<Void, Error>?
        let manager = BackgroundRegistration(defaults: defaults, status: { state }, unregister: {
            events.append("unregister")
            if state == .notFound { throw NSError(domain: "SMAppServiceErrorDomain", code: 3) }
            try await withCheckedThrowingContinuation { completion = $0 }
            state = .notRegistered
            events.append("unregistered")
        }, register: {
            precondition(state == .notRegistered || state == .notFound)
            events.append("register")
            state = .enabled
        })
        // An enabled service left over from an older executable must be refreshed.
        var migrationReturned = false
        let update = Task {
            defer { migrationReturned = true }
            try await manager.reconcile(enabled: true, revision: "v2")
        }
        while completion == nil && !migrationReturned { await Task.yield() }
        precondition(completion != nil, "an enabled job with stale revision must be re-registered")
        precondition(events == ["unregister"], "must await OS completion before registering")
        precondition(defaults.string(forKey: manager.key) == nil)
        completion!.resume(); completion = nil
        try await update.value
        precondition(events == ["unregister", "unregistered", "register"])
        precondition(defaults.string(forKey: manager.key) == "v2")
        events = []
        try await manager.reconcile(enabled: true, revision: "v2")
        precondition(events.isEmpty, "unchanged launches must not churn registration")
        // Failed repair must remain retryable and never register over a live job.
        let repair = Task { try await manager.reconcile(enabled: true, revision: "v2", force: true) }
        while completion == nil { await Task.yield() }
        completion!.resume(throwing: NSError(domain: "test", code: 1)); completion = nil
        do { try await repair.value; preconditionFailure("expected failure") } catch {}
        precondition(events == ["unregister"])
        precondition(defaults.string(forKey: manager.key) == nil)
        // User-denied approval must not be bypassed by an automatic update.
        state = .requiresApproval; events = []
        do { try await manager.reconcile(enabled: true, revision: "v3"); preconditionFailure() } catch {}
        precondition(events.isEmpty)
        // Rapid off/on requests must wait for the first removal to complete.
        state = .enabled
        let off = Task { try await manager.reconcile(enabled: false, revision: "v3") }
        while completion == nil { await Task.yield() }
        let on = Task { try await manager.reconcile(enabled: true, revision: "v3") }
        await Task.yield()
        precondition(events == ["unregister"])
        completion!.resume(); completion = nil
        try await off.value; try await on.value
        precondition(events == ["unregister", "unregistered", "register"])
        // macOS reports .notFound for a new label with no BTM record yet.
        // Unregistering that state fails with JobNotFound and prevents creation.
        state = .notFound; events = []
        try await manager.reconcile(enabled: true, revision: "new-label")
        precondition(events == ["register"], "new labels must register without unregistering")
        print("OK: new label, migration, completion ordering, unchanged startup, repair failure, approval, concurrent settings")
    }
}
