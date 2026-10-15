import Cocoa
import InputMethodKit
import os

let log = Logger(subsystem: "dev.noky.inputmethod.lightbopomofo", category: "ime")

/// One engine per process, loaded at startup.
enum SharedEngine {
    static let engine: Engine? = {
        guard let lexicon = Bundle.main.path(forResource: "tsi", ofType: "csv") else {
            log.error("lexicon tsi.csv missing from bundle")
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

    private static let punctuation: [String: String] = [
        "<": "，", ">": "。", "?": "？", "!": "！", ":": "：",
    ]

    override func recognizedEvents(_ sender: Any!) -> Int {
        Int(NSEvent.EventTypeMask.keyDown.rawValue)
    }

    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event, event.type == .keyDown, let composer,
              let client = sender as? IMKTextInput
        else { return false }
        let result = composer.handle(Self.key(for: event))
        if !result.commit.isEmpty {
            client.insertText(result.commit, replacementRange: NSRange(location: NSNotFound, length: 0))
            showingMarkedText = false
        }
        render(client)
        return result.handled
    }

    override func commitComposition(_ sender: Any!) {
        guard let composer, let client = sender as? IMKTextInput, !composer.isEmpty else { return }
        let text = composer.commitAll()
        client.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
        showingMarkedText = false
        render(client)
    }

    override func deactivateServer(_ sender: Any!) {
        commitComposition(sender)
        CandidatePanel.shared.hide()
        super.deactivateServer(sender)
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

    static func key(for event: NSEvent) -> Key {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        // Shortcuts, Caps Lock English and keypad numbers go to the app.
        // Arrow keys carry .numericPad too, so they are exempt from that one.
        let isArrow = (123...126).contains(event.keyCode)
        if !flags.isDisjoint(with: [.command, .control, .option, .capsLock])
            || (flags.contains(.numericPad) && !isArrow)
        {
            return .other
        }
        switch event.keyCode {
        case 49, 36, 76: return .commit  // space, return, keypad enter
        case 51: return .backspace
        case 53: return .escape
        case 123, 126: return .previous  // ←, ↑
        case 124, 125: return .next  // →, ↓
        case 48: return flags.contains(.shift) ? .previous : .next  // tab
        default: break
        }
        if flags.contains(.shift) {
            if let p = punctuation[event.characters ?? ""] { return .punctuation(p) }
            return .other
        }
        if let ch = event.charactersIgnoringModifiers?.lowercased().first,
           let symbol = Engine.bopomofo(forKey: ch)
        {
            return .symbol(symbol)
        }
        return .other
    }
}
