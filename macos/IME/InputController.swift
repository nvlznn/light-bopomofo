import Cocoa
import InputMethodKit
import os

let log = Logger(subsystem: "dev.noky.inputmethod.lightbopomofo", category: "ime")

/// One engine per process, loaded at startup.
enum SharedEngine {
    static let engine: Engine? = {
        guard let lexicon = Bundle.main.path(forResource: "lexicon", ofType: "bin") else {
            log.error("lexicon.bin missing from bundle")
            return nil
        }
        let engine = Engine(lexiconPath: lexicon, historyPath: UserData.historyPath)
        if engine == nil { log.error("engine failed to load \(lexicon, privacy: .public)") }
        return engine
    }()
}

/// Translates IMK events into `Key`s for the composer and renders its state.
@objc(LBInputController)
final class InputController: IMKInputController {
    private lazy var composer: Composer? = SharedEngine.engine.map(Composer.init)
    private var showingMarkedText = false
    /// A commit the system asked for, waiting to see if deactivateServer follows.
    private var pendingCommit: (work: DispatchWorkItem, client: IMKTextInput)?

    /// The input source's menu in the menu bar: a newer version the app
    /// found (it could not install it by itself), and the app.
    override func menu() -> NSMenu! {
        let menu = NSMenu()
        let current = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0"
        if let version = PendingUpdate.version(newerThan: current) {
            menu.addItem(withTitle: "更新到輕注音 \(version)…", action: #selector(update(_:)), keyEquivalent: "")
        }
        menu.addItem(withTitle: "輕注音設定…", action: #selector(openApp(_:)), keyEquivalent: "")
        return menu
    }

    @objc private func update(_ sender: Any?) {
        NSWorkspace.shared.open(PendingUpdate.url)
    }

    @objc private func openApp(_ sender: Any?) {
        guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "dev.noky.lightbopomofo") else { return }
        NSWorkspace.shared.openApplication(at: app, configuration: NSWorkspace.OpenConfiguration())
    }

    override func recognizedEvents(_ sender: Any!) -> Int {
        Int(NSEvent.EventTypeMask.keyDown.rawValue)
    }

    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event, event.type == .keyDown, let composer,
              let client = sender as? IMKTextInput
        else { return false }
        commitPending()
        let result = composer.handle(Key(event: event, composing: !composer.isEmpty))
        if !result.commit.isEmpty {
            client.insertText(result.commit, replacementRange: NSRange(location: NSNotFound, length: 0))
            showingMarkedText = false
        }
        render(client)
        return result.handled
    }

    /// The system asks to commit both when the app ends the composition (a
    /// click elsewhere…) and right before deactivateServer when the user
    /// switches input method (or app). So the commit waits a moment: if
    /// deactivateServer follows, the bopomofo not chosen yet is dropped.
    override func commitComposition(_ sender: Any!) {
        guard let composer, let client = sender as? IMKTextInput, !composer.isEmpty else { return }
        pendingCommit?.work.cancel()
        let work = DispatchWorkItem { [weak self] in self?.commitPending() }
        pendingCommit = (work, client)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15, execute: work)
    }

    override func deactivateServer(_ sender: Any!) {
        pendingCommit?.work.cancel()
        pendingCommit = nil
        if let composer, !composer.isEmpty, let client = sender as? IMKTextInput {
            log.info("deactivated: composition dropped")
            composer.reset()
            render(client)
        }
        CandidatePanel.shared.hide()
        super.deactivateServer(sender)
    }

    private func commitPending() {
        guard let (work, client) = pendingCommit else { return }
        work.cancel()
        pendingCommit = nil
        guard let composer, !composer.isEmpty else { return }
        client.insertText(composer.commitAll(), replacementRange: NSRange(location: NSNotFound, length: 0))
        showingMarkedText = false
        render(client)
    }

    private func render(_ client: IMKTextInput) {
        let notFound = NSRange(location: NSNotFound, length: 0)
        guard let composer, !composer.isEmpty else {
            if showingMarkedText {
                client.setMarkedText("", selectionRange: NSRange(location: 0, length: 0), replacementRange: notFound)
                showingMarkedText = false
            }
            CandidatePanel.shared.hide()
            return
        }
        let text = composer.composingText
        let marked = NSAttributedString(string: text, attributes: [
            .underlineStyle: NSUnderlineStyle.single.rawValue,
        ])
        let caretAt = (text as NSString).length
        client.setMarkedText(marked, selectionRange: NSRange(location: caretAt, length: 0), replacementRange: notFound)
        showingMarkedText = true

        var caret = NSRect.zero
        client.attributes(forCharacterIndex: 0, lineHeightRectangle: &caret)
        CandidatePanel.shared.show(composer.candidates.map(\.text), selected: composer.selected, caret: caret)
    }
}
