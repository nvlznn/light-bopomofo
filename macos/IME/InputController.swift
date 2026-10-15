import Carbon
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
        let result = composer.handle(Self.key(for: event, composing: !composer.isEmpty))
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
        // Switching apps commits; switching to another input method drops
        // the bopomofo not chosen yet.
        if Self.isSelected {
            commitComposition(sender)
        } else if let composer, !composer.isEmpty, let client = sender as? IMKTextInput {
            log.info("input method switched: composition dropped")
            composer.reset()
            render(client)
        }
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

    /// Whether this input method is still the selected one (not when the
    /// user switched to another).
    static var isSelected: Bool {
        guard let source = TISCopyCurrentKeyboardInputSource()?.takeRetainedValue(),
              let raw = TISGetInputSourceProperty(source, kTISPropertyBundleID)
        else { return true }
        let id = Unmanaged<CFString>.fromOpaque(raw).takeUnretainedValue() as String
        return id == Bundle.main.bundleIdentifier
    }

    /// Number keys 1–9 by key code (the keypad's are separate keys).
    private static let digits: [UInt16: Int] = [
        18: 1, 19: 2, 20: 3, 21: 4, 23: 5, 22: 6, 26: 7, 28: 8, 25: 9,
    ]

    static func key(for event: NSEvent, composing: Bool) -> Key {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        // Shortcuts and keypad numbers go to the app. Arrows, PageUp and
        // PageDown carry .numericPad too, so they are exempt from that one.
        let isNavigation = (123...126).contains(event.keyCode) || [116, 121].contains(event.keyCode)
        if !flags.isDisjoint(with: [.command, .control, .option])
            || (flags.contains(.numericPad) && !isNavigation)
        {
            return .other
        }
        if flags.contains(.capsLock) { return .abandon }  // Caps Lock types English
        switch event.keyCode {
        case 49: return .symbol(Composer.firstTone)  // space
        case 36, 76: return .commit  // return, keypad enter
        case 51: return .backspace
        case 53: return .escape
        case 123, 126: return .previous  // ←, ↑
        case 124, 125: return .next  // →, ↓
        case 48: return flags.contains(.shift) ? .previous : .next  // tab
        case 116: return .pagePrevious  // PageUp
        case 121: return .pageNext  // PageDown
        case 33 where !flags.contains(.shift): return .pagePrevious  // [
        case 30 where !flags.contains(.shift): return .pageNext  // ]
        default: break
        }
        if flags.contains(.shift) {
            if composing, let n = digits[event.keyCode] { return .select(n) }
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
