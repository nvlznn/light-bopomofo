import Cocoa

/// Horizontal candidate strip near the caret: up to 9 per page, numbered.
final class CandidatePanel {
    static let shared = CandidatePanel()
    static let perPage = 9

    private let panel: NSPanel
    private let view = CandidateView()
    private var lastOrigin: NSPoint?

    private init() {
        panel = NSPanel(
            contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered, defer: true)
        panel.level = .popUpMenu
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.ignoresMouseEvents = true
        panel.contentView = view
    }

    func show(_ candidates: [String], selected: Int, caret: NSRect) {
        guard !candidates.isEmpty else { return hide() }
        let page = selected / CandidatePanel.perPage
        let start = page * CandidatePanel.perPage
        let end = min(start + CandidatePanel.perPage, candidates.count)
        let pages = (candidates.count + CandidatePanel.perPage - 1) / CandidatePanel.perPage
        view.items = Array(candidates[start..<end])
        view.selected = selected - start
        view.pageLabel = pages > 1 ? "\(page + 1)/\(pages)" : nil
        let size = view.fittingSize
        panel.setContentSize(size)
        panel.setFrameOrigin(origin(for: size, caret: caret))
        view.needsDisplay = true
        panel.orderFrontRegardless()
    }

    func hide() {
        panel.orderOut(nil)
    }

    private func origin(for size: NSSize, caret: NSRect) -> NSPoint {
        // Some apps report no caret rect; reuse the last good position.
        if caret == .zero || (caret.origin == .zero && caret.height == 0) {
            if let lastOrigin { return lastOrigin }
            let frame = NSScreen.main?.visibleFrame ?? .zero
            return NSPoint(x: frame.midX - size.width / 2, y: frame.minY + 120)
        }
        let screen =
            NSScreen.screens.first { $0.frame.contains(caret.origin) } ?? NSScreen.main
        let visible = screen?.visibleFrame ?? .zero
        var origin = NSPoint(x: caret.minX, y: caret.minY - size.height - 4)
        if origin.y < visible.minY {
            origin.y = caret.maxY + 4  // no room below: show above the line
        }
        origin.x = min(max(origin.x, visible.minX), visible.maxX - size.width)
        lastOrigin = origin
        return origin
    }
}

private final class CandidateView: NSView {
    var items: [String] = []
    var selected = 0
    var pageLabel: String?

    private let textFont = NSFont.systemFont(ofSize: 18)
    private let indexFont = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .medium)
    private let padding: CGFloat = 6
    private let cellPadding: CGFloat = 7
    private let height: CGFloat = 34

    override var isFlipped: Bool { true }

    override var fittingSize: NSSize {
        var width = padding * 2
        for (i, _) in items.enumerated() { width += cellWidth(i) }
        if let pageLabel { width += label(pageLabel, font: indexFont, color: .secondaryLabelColor).size().width + cellPadding * 2 }
        return NSSize(width: ceil(width), height: height)
    }

    override func draw(_ dirtyRect: NSRect) {
        let background = NSBezierPath(roundedRect: bounds, xRadius: 8, yRadius: 8)
        NSColor.windowBackgroundColor.setFill()
        background.fill()
        NSColor.separatorColor.setStroke()
        background.stroke()

        var x = padding
        for (i, text) in items.enumerated() {
            let w = cellWidth(i)
            let isSelected = i == selected
            if isSelected {
                let cell = NSRect(x: x, y: 4, width: w, height: height - 8)
                NSColor.selectedContentBackgroundColor.setFill()
                NSBezierPath(roundedRect: cell, xRadius: 5, yRadius: 5).fill()
            }
            let fg: NSColor = isSelected ? .alternateSelectedControlTextColor : .labelColor
            let dim: NSColor = isSelected ? .alternateSelectedControlTextColor : .secondaryLabelColor
            let index = label("\(i + 1)", font: indexFont, color: dim)
            let word = label(text, font: textFont, color: fg)
            index.draw(at: NSPoint(x: x + cellPadding, y: (height - index.size().height) / 2))
            word.draw(at: NSPoint(
                x: x + cellPadding + index.size().width + 3,
                y: (height - word.size().height) / 2))
            x += w
        }
        if let pageLabel {
            let page = label(pageLabel, font: indexFont, color: .secondaryLabelColor)
            page.draw(at: NSPoint(x: x + cellPadding, y: (height - page.size().height) / 2))
        }
    }

    private func cellWidth(_ i: Int) -> CGFloat {
        let index = label("\(i + 1)", font: indexFont, color: .labelColor).size().width
        let word = label(items[i], font: textFont, color: .labelColor).size().width
        return index + 3 + word + cellPadding * 2
    }

    private func label(_ s: String, font: NSFont, color: NSColor) -> NSAttributedString {
        NSAttributedString(string: s, attributes: [.font: font, .foregroundColor: color])
    }
}
