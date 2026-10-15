// Draws the icons.
//   swift scripts/make-icon.swift menu macos/IME/Resources/icon.tiff
//       input-source icon: black glyph on transparent (template), 16 + 32 px
//   swift scripts/make-icon.swift app macos/App/Resources/AppIcon.icns
//       app icon: white rounded square with a black glyph
//   swift scripts/make-icon.swift ico windows/lightbopomofo.ico
//       the same, as a Windows icon (16–256 px)
import AppKit

func bitmap(_ px: Int, points: CGFloat, draw: () -> Void) -> NSBitmapImageRep {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: points, height: points)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    draw()
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

func glyph(size: CGFloat, color: NSColor, in box: CGFloat, weight: NSFont.Weight, dy: CGFloat) {
    let text = NSAttributedString(string: "輕", attributes: [
        .font: NSFont.systemFont(ofSize: size, weight: weight), .foregroundColor: color,
    ])
    let s = text.size()
    text.draw(at: NSPoint(x: (box - s.width) / 2, y: (box - s.height) / 2 + dy))
}

func menuIcon(_ out: String) {
    let image = NSImage(size: NSSize(width: 16, height: 16))
    for px in [16, 32] {
        image.addRepresentation(bitmap(px, points: 16) {
            glyph(size: 14.5, color: .black, in: 16, weight: .medium, dy: 0.5)
        })
    }
    try! image.tiffRepresentation(using: .lzw, factor: 1)!.write(to: URL(fileURLWithPath: out))
}

/// The app icon at `px` pixels: white rounded square, black glyph.
func appIconRep(_ px: Int) -> NSBitmapImageRep {
    bitmap(px, points: CGFloat(px)) {
        // macOS icon grid: 824/1024 body with ~185/1024 corner radius.
        let side = CGFloat(px), inset = side * 100 / 1024
        let body = NSRect(x: inset, y: inset, width: side - inset * 2, height: side - inset * 2)
        let radius = body.width * 0.225
        NSGraphicsContext.saveGraphicsState()
        let shadow = NSShadow()
        shadow.shadowColor = NSColor.black.withAlphaComponent(0.25)
        shadow.shadowBlurRadius = side * 0.02
        shadow.shadowOffset = NSSize(width: 0, height: -side * 0.01)
        shadow.set()
        let shape = NSBezierPath(roundedRect: body, xRadius: radius, yRadius: radius)
        NSColor.white.setFill()
        shape.fill()
        NSGraphicsContext.restoreGraphicsState()
        // A faint edge so the white body holds its shape on white backgrounds.
        NSColor(white: 0, alpha: 0.12).setStroke()
        shape.lineWidth = max(1, side / 512)
        shape.stroke()
        glyph(size: body.width * 0.62, color: .black, in: side, weight: .semibold, dy: side * 0.01)
    }
}

func appIcon(_ out: String) {
    let iconset = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("AppIcon.iconset")
    try? FileManager.default.removeItem(at: iconset)
    try! FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
    for points in [16, 32, 128, 256, 512] {
        for scale in [1, 2] {
            let rep = appIconRep(points * scale)
            let name = scale == 1 ? "icon_\(points)x\(points).png" : "icon_\(points)x\(points)@2x.png"
            try! rep.representation(using: .png, properties: [:])!.write(to: iconset.appendingPathComponent(name))
        }
    }
    let iconutil = Process()
    iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
    iconutil.arguments = ["-c", "icns", iconset.path, "-o", out]
    try! iconutil.run()
    iconutil.waitUntilExit()
}

/// Windows .ico: a directory of PNG images (supported since Windows Vista).
func windowsIcon(_ out: String) {
    let sizes = [16, 20, 24, 32, 40, 48, 64, 256]
    let pngs = sizes.map { appIconRep($0).representation(using: .png, properties: [:])! }
    var data = Data()
    func u16(_ v: Int) { data.append(contentsOf: [UInt8(v & 0xff), UInt8(v >> 8 & 0xff)]) }
    func u32(_ v: Int) { u16(v & 0xffff); u16(v >> 16) }
    u16(0); u16(1); u16(sizes.count)  // reserved, type = icon, count
    var offset = 6 + 16 * sizes.count
    for (size, png) in zip(sizes, pngs) {
        data.append(UInt8(size >= 256 ? 0 : size))  // width (0 = 256)
        data.append(UInt8(size >= 256 ? 0 : size))  // height
        data.append(0); data.append(0)  // palette, reserved
        u16(1); u16(32)  // planes, bits per pixel
        u32(png.count); u32(offset)
        offset += png.count
    }
    pngs.forEach { data.append($0) }
    try! data.write(to: URL(fileURLWithPath: out))
}

switch CommandLine.arguments[1] {
case "menu": menuIcon(CommandLine.arguments[2])
case "app": appIcon(CommandLine.arguments[2])
case "ico": windowsIcon(CommandLine.arguments[2])
default: fatalError("usage: make-icon.swift menu|app|ico OUTPUT")
}
