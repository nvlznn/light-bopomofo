// Draws the disk-image window background: an arrow from the app (left) to
// Applications (right) and bopomofo symbols scattered around them.
// Usage: swift scripts/make-dmg-background.swift macos/dmg/background.tiff
//
// Layout must match macos/dmg/settings.py (window 660×420, 128 pt icons
// centred at (170, 200) and (490, 200), measured from the top left).
import AppKit

let width: CGFloat = 660, height: CGFloat = 420
let appCenter = CGPoint(x: 170, y: 200), applicationsCenter = CGPoint(x: 490, y: 200)

/// Window coordinates (origin top left) → drawing coordinates (bottom left).
func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: x, y: height - y) }

let ink = NSColor(srgbRed: 0.16, green: 0.15, blue: 0.14, alpha: 1)
let symbols = Array("ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ")  // tone marks read as specks

/// Areas the scattered symbols stay out of (window coordinates).
let keepClear: [CGRect] = [
    CGRect(x: appCenter.x - 95, y: appCenter.y - 90, width: 190, height: 205),  // icon + name
    CGRect(x: applicationsCenter.x - 95, y: applicationsCenter.y - 90, width: 190, height: 205),
    CGRect(x: 238, y: 140, width: 184, height: 110),  // arrow
    CGRect(x: 150, y: 352, width: 360, height: 44),  // hint
]

/// Deterministic pseudo-random numbers, so the image is the same every run.
struct Random {
    var state: UInt64 = 0x5EED_B0B0
    mutating func next() -> CGFloat {
        state = state &* 6364136223846793005 &+ 1442695040888963407
        return CGFloat(state >> 33) / CGFloat(1 << 31)
    }
    mutating func range(_ a: CGFloat, _ b: CGFloat) -> CGFloat { a + (b - a) * next() }
}

func drawBackground() {
    NSColor(srgbRed: 0.965, green: 0.953, blue: 0.933, alpha: 1).setFill()
    NSRect(x: 0, y: 0, width: width, height: height).fill()

    // Scattered symbols.
    var rng = Random()
    var placed: [(CGPoint, CGFloat)] = []
    var attempts = 0
    while placed.count < 64 && attempts < 4000 {
        attempts += 1
        let size = rng.range(15, 38)
        let center = CGPoint(x: rng.range(14, width - 14), y: rng.range(14, height - 14))
        let box = CGRect(x: center.x - size / 2, y: center.y - size / 2, width: size, height: size)
        if keepClear.contains(where: { $0.intersects(box) }) { continue }
        if placed.contains(where: { hypot($0.0.x - center.x, $0.0.y - center.y) < ($0.1 + size) * 0.62 }) {
            continue
        }
        placed.append((center, size))
        let symbol = symbols[Int(rng.next() * CGFloat(symbols.count)) % symbols.count]
        let alpha = rng.range(0.07, 0.2)
        let angle = rng.range(-24, 24)
        let text = NSAttributedString(string: String(symbol), attributes: [
            .font: NSFont.systemFont(ofSize: size, weight: .medium),
            .foregroundColor: ink.withAlphaComponent(alpha),
        ])
        let s = text.size()
        NSGraphicsContext.saveGraphicsState()
        let t = NSAffineTransform()
        let at = p(center.x, center.y)
        t.translateX(by: at.x, yBy: at.y)
        t.rotate(byDegrees: angle)
        t.concat()
        text.draw(at: NSPoint(x: -s.width / 2, y: -s.height / 2))
        NSGraphicsContext.restoreGraphicsState()
    }

    // The arrow: a playful curve from the app towards Applications.
    let start = p(252, 214), end = p(404, 196)
    let c1 = p(292, 146), c2 = p(338, 266)
    let curve = NSBezierPath()
    curve.move(to: start)
    curve.curve(to: end, controlPoint1: c1, controlPoint2: c2)
    curve.lineWidth = 5
    curve.lineCapStyle = .round
    ink.withAlphaComponent(0.85).setStroke()
    curve.stroke()
    let angle = atan2(end.y - c2.y, end.x - c2.x)
    let head = NSBezierPath()
    let length: CGFloat = 20, spread: CGFloat = 0.5
    head.move(to: CGPoint(x: end.x + 6 * cos(angle), y: end.y + 6 * sin(angle)))
    head.line(to: CGPoint(x: end.x - length * cos(angle - spread), y: end.y - length * sin(angle - spread)))
    head.line(to: CGPoint(x: end.x - length * cos(angle + spread), y: end.y - length * sin(angle + spread)))
    head.close()
    ink.withAlphaComponent(0.85).setFill()
    head.fill()

    // Hint.
    let hint = NSAttributedString(string: "把「輕注音」拖到「應用程式」就完成安裝", attributes: [
        .font: NSFont.systemFont(ofSize: 14, weight: .medium),
        .foregroundColor: NSColor(srgbRed: 0.42, green: 0.40, blue: 0.37, alpha: 1),
    ])
    let hs = hint.size()
    hint.draw(at: p((width - hs.width) / 2, 374 + hs.height / 2))
}

func rep(scale: CGFloat) -> NSBitmapImageRep {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: Int(width * scale), pixelsHigh: Int(height * scale),
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: width, height: height)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    drawBackground()
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

let image = NSImage(size: NSSize(width: width, height: height))
image.addRepresentation(rep(scale: 1))
image.addRepresentation(rep(scale: 2))
let out = URL(fileURLWithPath: CommandLine.arguments[1])
try! FileManager.default.createDirectory(at: out.deletingLastPathComponent(), withIntermediateDirectories: true)
try! image.tiffRepresentation(using: .lzw, factor: 1)!.write(to: out)
