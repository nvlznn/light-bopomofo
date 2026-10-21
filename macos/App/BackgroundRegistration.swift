import Foundation
import ServiceManagement

/// The same reconciliation path is used by startup, settings and explicit repair.
/// Keep the OS operations injectable so tests can hold unregister completion open.
@MainActor
final class BackgroundRegistration {
    let defaults: UserDefaults
    let key = "BackgroundServiceRevision"
    var status: () -> SMAppService.Status
    var unregister: () async throws -> Void
    var register: () throws -> Void
    private var pending: Task<Void, Error>?

    init(defaults: UserDefaults, status: @escaping () -> SMAppService.Status,
         unregister: @escaping () async throws -> Void, register: @escaping () throws -> Void) {
        self.defaults = defaults
        self.status = status
        self.unregister = unregister
        self.register = register
    }

    func reconcile(enabled: Bool, revision: String, force: Bool = false) async throws {
        let previous = pending
        let task = Task { @MainActor in
            _ = try? await previous?.value
            try await self.apply(enabled: enabled, revision: revision, force: force)
        }
        pending = task
        try await task.value
    }

    private func apply(enabled: Bool, revision: String, force: Bool) async throws {
        if !enabled {
            if status() == .enabled || status() == .requiresApproval { try await unregister() }
            defaults.removeObject(forKey: key)
            return
        }
        // Approval is the user's choice. Automatic migrations must not reset it.
        if status() == .requiresApproval { throw RegistrationError.approval }
        let changed = defaults.string(forKey: key) != revision
        if force || changed {
            defaults.removeObject(forKey: key)
            if status() == .enabled || status() == .requiresApproval { try await unregister() }
        }
        if status() != .enabled { try register() }
        guard status() == .enabled else { throw RegistrationError.approval }
        defaults.set(revision, forKey: key)
    }
}

enum RegistrationError: LocalizedError {
    case approval, developmentCopy, handoffTimeout
    var errorDescription: String? {
        switch self {
        case .approval: return "請在系統設定的「登入項目與延伸功能」允許輕注音在背景執行，再重試。"
        case .developmentCopy: return "請從「應用程式」開啟正式簽署的輕注音，再修復背景服務。"
        case .handoffTimeout: return "舊的背景工作尚未結束，請從輕注音設定修復背景服務。"
        }
    }
}
