// Draws the disk-image window background: a bright gradient, colourful
// bopomofo and confetti all around, a bold arrow from the app (left) to
// Applications (right), and soft pads behind both icons and their names.
// Usage: swift scripts/make-dmg-background.swift macos/dmg/background.tiff
//
// Layout must match macos/dmg/settings.py (window 660×420, 128 pt icons
// centred at (170, 200) and (490, 200), measured from the top left).
import AppKit

let width: CGFloat = 660, height: CGFloat = 420
let icons = [CGPoint(x: 170, y: 200), CGPoint(x: 490, y: 200)]

/// Window coordinates (origin top left) → drawing coordinates (bottom left).
func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: x, y: height - y) }
func rgb(_ hex: UInt32, _ alpha: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: CGFloat(hex >> 16 & 0xff) / 255, green: CGFloat(hex >> 8 & 0xff) / 255,
            blue: CGFloat(hex & 0xff) / 255, alpha: alpha)
}

let palette: [NSColor] = [
    rgb(0xFF5A5F), rgb(0xFF9F1C), rgb(0xF4B400), rgb(0x2EC4B6),
    rgb(0x3A86FF), rgb(0x8338EC), rgb(0xFF4FA3), rgb(0x06A77D),
]
let symbols = Array("ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ")

/// Areas the scattered symbols stay out of (window coordinates).
let keepClear: [CGRect] = icons.map { CGRect(x: $0.x - 92, y: $0.y - 86, width: 184, height: 200) } + [
    CGRect(x: 240, y: 138, width: 180, height: 116),  // arrow
    CGRect(x: 120, y: 350, width: 420, height: 50),  // hint
]

/// Deterministic pseudo-random numbers, so the image is the same every run.
struct Random {
    var state: UInt64 = 0x5EED_B0B0
    mutating func next() -> CGFloat {
        state = state &* 6364136223846793005 &+ 1442695040888963407
        return CGFloat(state >> 33) / CGFloat(1 << 31)
    }
    mutating func range(_ a: CGFloat, _ b: CGFloat) -> CGFloat { a + (b - a) * next() }
    mutating func pick<T>(_ xs: [T]) -> T { xs[Int(next() * CGFloat(xs.count)) % xs.count] }
}

func drawBackground() {
    let bounds = NSRect(x: 0, y: 0, width: width, height: height)
    NSGradient(colors: [rgb(0xFFF0B3), rgb(0xFFD6C4), rgb(0xFBC9E6), rgb(0xD9CFFF)])!
        .draw(in: bounds, angle: -28)

    var rng = Random()

    // Confetti dots first, so symbols sit on top of them.
    for _ in 0..<70 {
        let r = rng.range(2.5, 6)
        let c = CGPoint(x: rng.range(8, width - 8), y: rng.range(8, height - 8))
        let box = CGRect(x: c.x - r, y: c.y - r, width: r * 2, height: r * 2)
        if keepClear.contains(where: { $0.intersects(box) }) { continue }
        rng.pick(palette).withAlphaComponent(rng.range(0.45, 0.8)).setFill()
        let at = p(c.x, c.y)
        NSBezierPath(ovalIn: NSRect(x: at.x - r, y: at.y - r, width: r * 2, height: r * 2)).fill()
    }

    // Colourful bopomofo all around.
    var placed: [(CGPoint, CGFloat)] = []
    var attempts = 0
    while placed.count < 60 && attempts < 6000 {
        attempts += 1
        let size = rng.range(20, 48)
        let center = CGPoint(x: rng.range(16, width - 16), y: rng.range(16, height - 16))
        let box = CGRect(x: center.x - size / 2, y: center.y - size / 2, width: size, height: size)
        if keepClear.contains(where: { $0.intersects(box) }) { continue }
        if placed.contains(where: { hypot($0.0.x - center.x, $0.0.y - center.y) < ($0.1 + size) * 0.6 }) {
            continue
        }
        placed.append((center, size))
        let text = NSAttributedString(string: String(rng.pick(symbols)), attributes: [
            .font: NSFont.systemFont(ofSize: size, weight: rng.next() < 0.5 ? .bold : .heavy),
            .foregroundColor: rng.pick(palette).withAlphaComponent(rng.range(0.7, 0.95)),
        ])
        let s = text.size()
        NSGraphicsContext.saveGraphicsState()
        let t = NSAffineTransform()
        let at = p(center.x, center.y)
        t.translateX(by: at.x, yBy: at.y)
        t.rotate(byDegrees: rng.range(-28, 28))
        t.concat()
        text.draw(at: NSPoint(x: -s.width / 2, y: -s.height / 2))
        NSGraphicsContext.restoreGraphicsState()
    }

    // Soft white glow behind each icon and a pad behind its name, so the
    // icons pop and Finder's labels stay readable on the colours.
    for c in icons {
        let glow = NSGradient(colors: [NSColor.white.withAlphaComponent(0.85), NSColor.white.withAlphaComponent(0)])!
        let at = p(c.x, c.y)
        glow.draw(fromCenter: at, radius: 0, toCenter: at, radius: 100, options: [])
        let pad = NSRect(x: at.x - 78, y: at.y - 98, width: 156, height: 28)
        NSColor.white.withAlphaComponent(0.8).setFill()
        NSBezierPath(roundedRect: pad, xRadius: 14, yRadius: 14).fill()
    }

    // The arrow: a bold, playful curve with a white outline.
    let start = p(254, 212), end = p(400, 196)
    let c1 = p(292, 140), c2 = p(338, 270)
    let curve = NSBezierPath()
    curve.move(to: start)
    curve.curve(to: end, controlPoint1: c1, controlPoint2: c2)
    curve.lineCapStyle = .round
    let angle = atan2(end.y - c2.y, end.x - c2.x)
    let head = NSBezierPath()
    let length: CGFloat = 24, spread: CGFloat = 0.55
    head.move(to: CGPoint(x: end.x + 8 * cos(angle), y: end.y + 8 * sin(angle)))
    head.line(to: CGPoint(x: end.x - length * cos(angle - spread), y: end.y - length * sin(angle - spread)))
    head.line(to: CGPoint(x: end.x - length * cos(angle + spread), y: end.y - length * sin(angle + spread)))
    head.close()
    head.lineJoinStyle = .round
    NSColor.white.setStroke()
    curve.lineWidth = 13
    curve.stroke()
    head.lineWidth = 7
    head.stroke()
    let coral = rgb(0xFF4F7B)
    coral.setStroke()
    curve.lineWidth = 7
    curve.stroke()
    coral.setFill()
    head.fill()

    // Hint.
    let hint = NSAttributedString(string: "把「輕注音」拖到「應用程式」就完成安裝！", attributes: [
        .font: NSFont.systemFont(ofSize: 15, weight: .bold),
        .foregroundColor: rgb(0x4B2E83),
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
