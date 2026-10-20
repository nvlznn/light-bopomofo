import Cocoa

/// The candidates near the caret: a strip of up to 9 per page, numbered, or
/// (after ↓) a grid of 6 columns, numbered along the selected row.
final class CandidatePanel {
    static let shared = CandidatePanel()
    static let perPage = Composer.pageSize

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

    /// `grid`: the first visible row of the grid, or nil for the strip.
    func show(_ candidates: [String], selected: Int, grid: Int?, caret: NSRect) {
        guard !candidates.isEmpty else { return hide() }
        if let top = grid {
            let columns = Composer.gridColumns
            let start = top * columns
            let end = min(start + columns * Composer.gridRows, candidates.count)
            view.rows = stride(from: start, to: end, by: columns).map {
                Array(candidates[$0..<min($0 + columns, end)])
            }
            view.selected = (row: (selected - start) / columns, column: (selected - start) % columns)
            view.pageLabel = nil
            view.more = end < candidates.count
            view.firstRow = top
        } else {
            let page = selected / CandidatePanel.perPage
            let start = page * CandidatePanel.perPage
            let end = min(start + CandidatePanel.perPage, candidates.count)
            let pages = (candidates.count + CandidatePanel.perPage - 1) / CandidatePanel.perPage
            view.rows = [Array(candidates[start..<end])]
            view.selected = (row: 0, column: selected - start)
            view.pageLabel = pages > 1 ? "\(page + 1)/\(pages)" : nil
            view.more = candidates.count > 1
        }
        view.isGrid = grid != nil
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
    var rows: [[String]] = []
    var selected = (row: 0, column: 0)
    var pageLabel: String?
    var isGrid = false
    /// Strip: more candidates than shown (↓ expands); grid: more rows below.
    var more = false
    /// Grid: the number of its first visible row (0-based), for the row
    /// numbers down the left.
    var firstRow = 0

    private let textFont = NSFont.systemFont(ofSize: 18)
    private let indexFont = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .medium)
    private let padding: CGFloat = 6
    private let cellPadding: CGFloat = 7
    private let rowHeight: CGFloat = 34
    private let hint = "⌄"

    override var isFlipped: Bool { true }

    /// Grid: the column of row numbers, apart from the cells' own numbers.
    private var gutter: CGFloat {
        guard isGrid else { return 0 }
        return size("\(firstRow + rows.count)", indexFont).width + cellPadding * 2 + 4
    }

    override var fittingSize: NSSize {
        var width = padding * 2 + gutter + columnWidths.reduce(0, +)
        if !isGrid {
            if let pageLabel { width += size(pageLabel, indexFont).width + cellPadding * 2 }
            if more { width += size(hint, indexFont).width + cellPadding * 2 }
        }
        let extra: CGFloat = isGrid && more ? 14 : 0
        return NSSize(width: ceil(width), height: rowHeight * CGFloat(rows.count) + extra)
    }

    /// Strip: each cell fits its text; grid: each column fits its widest cell.
    private var columnWidths: [CGFloat] {
        let columns = rows.map(\.count).max() ?? 0
        return (0..<columns).map { c in
            rows.compactMap { $0.indices.contains(c) ? cellWidth(c, $0[c]) : nil }.max() ?? 0
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        let background = NSBezierPath(roundedRect: bounds, xRadius: 8, yRadius: 8)
        NSColor.windowBackgroundColor.setFill()
        background.fill()
        NSColor.separatorColor.setStroke()
        background.stroke()

        let widths = columnWidths
        let gutter = self.gutter
        if isGrid {
            NSColor.separatorColor.setFill()
            NSRect(x: padding + gutter - 4, y: 6, width: 0.5, height: rowHeight * CGFloat(rows.count) - 12).fill()
        }
        for (r, row) in rows.enumerated() {
            let y = CGFloat(r) * rowHeight
            if r > 0 {
                NSColor.separatorColor.setFill()
                NSRect(x: padding + gutter, y: y, width: bounds.width - padding * 2 - gutter, height: 0.5).fill()
            }
            if isGrid {
                let color: NSColor = r == selected.row ? .labelColor : .tertiaryLabelColor
                let number = label("\(firstRow + r + 1)", indexFont, color)
                number.draw(at: NSPoint(
                    x: padding + (gutter - 4 - number.size().width) / 2,
                    y: y + (rowHeight - number.size().height) / 2))
            }
            var x = padding + gutter
            for (c, text) in row.enumerated() {
                let w = widths[c]
                let isSelected = (r, c) == selected
                if isSelected {
                    NSColor.selectedContentBackgroundColor.setFill()
                    NSBezierPath(roundedRect: NSRect(x: x, y: y + 4, width: w, height: rowHeight - 8),
                                 xRadius: 5, yRadius: 5).fill()
                }
                let fg: NSColor = isSelected ? .alternateSelectedControlTextColor : .labelColor
                let dim: NSColor = isSelected ? .alternateSelectedControlTextColor : .secondaryLabelColor
                var textX = x + cellPadding
                if r == selected.row {  // numbers go with the selected row (Shift + 1–9)
                    let index = label("\(c + 1)", indexFont, dim)
                    index.draw(at: NSPoint(x: textX, y: y + (rowHeight - index.size().height) / 2))
                    textX += index.size().width + 3
                } else {
                    textX += size("\(c + 1)", indexFont).width + 3
                }
                let word = label(text, textFont, fg)
                word.draw(at: NSPoint(x: textX, y: y + (rowHeight - word.size().height) / 2))
                x += w
            }
            if !isGrid {
                if let pageLabel {
                    let page = label(pageLabel, indexFont, .secondaryLabelColor)
                    page.draw(at: NSPoint(x: x + cellPadding, y: (rowHeight - page.size().height) / 2))
                    x += page.size().width + cellPadding * 2
                }
                if more {
                    let more = label(hint, indexFont, .tertiaryLabelColor)
                    more.draw(at: NSPoint(x: x + cellPadding, y: (rowHeight - more.size().height) / 2))
                }
            }
        }
        if isGrid && more {
            let more = label(hint, indexFont, .tertiaryLabelColor)
            more.draw(at: NSPoint(x: (bounds.width - more.size().width) / 2, y: bounds.height - 15))
        }
    }

    private func cellWidth(_ column: Int, _ text: String) -> CGFloat {
        size("\(column + 1)", indexFont).width + 3 + size(text, textFont).width + cellPadding * 2
    }

    private func size(_ s: String, _ font: NSFont) -> NSSize {
        label(s, font, .labelColor).size()
    }

    private func label(_ s: String, _ font: NSFont, _ color: NSColor) -> NSAttributedString {
        NSAttributedString(string: s, attributes: [.font: font, .foregroundColor: color])
    }
}
