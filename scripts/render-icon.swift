import AppKit

// Render the project's original vector icon at each macOS icon size.
let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let packaging = root.appendingPathComponent("packaging")
let svg = try String(contentsOf: packaging.appendingPathComponent("amex-app-icon.svg"), encoding: .utf8)
guard let source = NSImage(data: Data(svg.utf8)) else { fatalError("Could not render the logo SVG") }

func png(size: Int) -> Data {
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    let canvas = NSRect(x: 0, y: 0, width: size, height: size)
    NSColor.clear.setFill()
    canvas.fill(using: .copy)
    source.draw(in: canvas, from: .zero, operation: .sourceOver, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()
    return bitmap.representation(using: .png, properties: [:])!
}

try png(size: 1024).write(to: packaging.appendingPathComponent("amex-icon.png"))
let temporary = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
let iconset = temporary.appendingPathComponent("AmexTogether.iconset")
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: temporary) }
for size in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let suffix = scale == 2 ? "@2x" : ""
        try png(size: size * scale).write(to: iconset.appendingPathComponent("icon_\(size)x\(size)\(suffix).png"))
    }
}
let converter = Process()
converter.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
converter.arguments = ["--convert", "icns", iconset.path, "--output", packaging.appendingPathComponent("AmexTogether.icns").path]
try converter.run()
converter.waitUntilExit()
guard converter.terminationStatus == 0 else { fatalError("Could not build the macOS icon") }
