import AppKit
import IrisCore
import IrisEngine
import SwiftUI

/// Renders UI parts for `--snapshot` and the other design checks: through a real
/// NSHostingView in an offscreen window (`hosted`), so AppKit-backed controls (pickers,
/// switches, fields) draw too, or through SwiftUI's own renderer (`pure`). Light only:
/// Iris has no dark theme.
@MainActor
enum ComponentSnapshot {
    static func render<V: View>(_ view: V, width: CGFloat, to path: String, design: DesignState = .snapshot) {
        let appearance = NSAppearance(named: .aqua)!
        let root = view.frame(width: width).fixedSize(horizontal: false, vertical: true)
            .environment(\.colorScheme, .light).environment(\.designState, design)
        let host = NSHostingView(rootView: root)
        host.appearance = appearance
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: width, height: 100),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.appearance = appearance
        window.contentView = host
        let size = host.fittingSize
        window.setContentSize(size)
        host.frame = NSRect(origin: .zero, size: size)
        host.layoutSubtreeIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.15))
        host.display()
        guard let rep = host.bitmapImageRepForCachingDisplay(in: host.bounds) else { return }
        appearance.performAsCurrentDrawingAppearance { host.cacheDisplay(in: host.bounds, to: rep) }
        if let png = rep.representation(using: .png, properties: [:]) { write(png, to: path) }
    }

    /// SwiftUI's own renderer, for views with no AppKit controls. Unlike cacheDisplay it
    /// draws blur and soft shadows, so the panel's paper shadow shows up.
    static func renderPure<V: View>(_ view: V, width: CGFloat, to path: String) {
        let renderer = ImageRenderer(content: view.frame(width: width).fixedSize(horizontal: false, vertical: true)
            .environment(\.colorScheme, .light).environment(\.designState, .snapshot))
        renderer.scale = 2
        guard let image = renderer.nsImage, let tiff = image.tiffRepresentation,
            let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:])
        else { return }
        write(png, to: path)
    }

    /// Writes one picture, making its folder first. A picture that can't be written stops the
    /// run, so a design check never quietly leaves pictures out.
    static func write(_ png: Data, to path: String) {
        let url = URL(fileURLWithPath: path)
        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try png.write(to: url)
        } catch {
            FileHandle.standardError.write(Data("couldn't write \(path): \(error.localizedDescription)\n".utf8))
            exit(1)
        }
        print(path)
    }

    static func pure<V: View>(_ name: String, width: CGFloat, dir: String, prefix: String, @ViewBuilder _ view: () -> V) {
        renderPure(view(), width: width, to: "\(dir)/\(prefix)-\(name)-light.png")
    }

    static func hosted<V: View>(_ name: String, width: CGFloat, dir: String, prefix: String, design: DesignState = .snapshot,
                                @ViewBuilder _ view: () -> V) {
        render(view(), width: width, to: "\(dir)/\(prefix)-\(name)-light.png", design: design)
    }
}
