// Draws the input method icon (template: black glyph on transparent).
// Usage: swift scripts/make-icon.swift macos/Resources/icon.tiff
import AppKit

func rep(_ px: Int) -> NSBitmapImageRep {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: 16, height: 16)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let font = NSFont.systemFont(ofSize: 14.5, weight: .medium)
    let glyph = NSAttributedString(string: "輕", attributes: [.font: font, .foregroundColor: NSColor.black])
    let size = glyph.size()
    glyph.draw(at: NSPoint(x: (16 - size.width) / 2, y: (16 - size.height) / 2 + 0.5))
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

let image = NSImage(size: NSSize(width: 16, height: 16))
image.addRepresentation(rep(16))
image.addRepresentation(rep(32))
try! image.tiffRepresentation(using: .lzw, factor: 1)!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
