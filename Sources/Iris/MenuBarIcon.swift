import AppKit
import SwiftUI

/// The menu-bar mark: the logo's iris as a child colours it in with a crayon, three petals
/// scribbled in level back-and-forth strokes like lines of handwriting, no outline, over a pink
/// plate printed a little up-left, off register; a stem and a quick leaf.
///
/// Not a template image, so the pink survives: the image draws itself each time the bar does
/// and picks white or black ink from the bar's appearance, the way a template would.
/// Drawn in mockups/logos/crayon.js (D1, "Written") on the logo mockups branch
/// (`t3/iris-logo-concept-mockups`, not in this checkout); the points below are copied from there.
enum MenuBarIcon {
    static let size = NSSize(width: 18, height: 18)

    static let flower: NSImage = {
        let image = NSImage(size: size, flipped: true) { _ in
            guard let ctx = NSGraphicsContext.current?.cgContext else { return false }
            let dark = NSAppearance.currentDrawing().bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            draw(in: ctx, ink: dark ? NSColor.white.cgColor : NSColor(white: 0, alpha: 0.85).cgColor)
            return true
        }
        image.isTemplate = false
        image.accessibilityDescription = "Iris"
        return image
    }()

    static let pink = DS.nsColor(Riso.pink).cgColor

    /// Draws into an 18×18 context with a top-left origin.
    static func draw(in ctx: CGContext, ink: CGColor) {
        // The pink plate, off register.
        ctx.saveGState()
        ctx.translateBy(x: -0.8, y: -0.6)
        ctx.setFillColor(pink)
        for p in plates { ctx.addPath(smoothPath(p, closed: true)) }
        ctx.fillPath()
        ctx.restoreGState()
        // The crayon.
        ctx.setStrokeColor(ink)
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)
        ctx.setLineWidth(0.7)
        for h in hatches { ctx.addPath(smoothPath(h)) }
        ctx.strokePath()
        ctx.setLineWidth(1.25)
        ctx.addPath(smoothPath(stem))
        ctx.strokePath()
        ctx.setLineWidth(1.05)
        ctx.addPath(smoothPath(leaf))
        ctx.strokePath()
    }

    /// The three petals' plates: the upright one, then the two falls, each a slightly wobbly oval.
    private static let plates: [[CGPoint]] = [
        [(11.2, 4.7), (10.97, 6.19), (10.43, 7.5), (9.54, 8.39), (8.45, 8.48), (7.5, 7.65), (6.96, 6.24), (6.86, 4.7), (7.08, 3.25), (7.61, 1.97), (8.47, 1.1), (9.54, 1.01), (10.47, 1.82), (10.99, 3.2)],
        [(8.21, 7.28), (8.06, 8.35), (7.33, 9.38), (6.26, 10.27), (4.92, 10.96), (3.41, 11.29), (2.12, 11.06), (1.47, 10.28), (1.62, 9.24), (2.34, 8.23), (3.38, 7.37), (4.68, 6.7), (6.15, 6.37), (7.42, 6.59)],
        [(16.39, 10.22), (15.58, 10.81), (14.46, 11.07), (13.08, 10.99), (11.57, 10.44), (10.34, 9.46), (9.83, 8.33), (10.09, 7.42), (10.88, 6.85), (11.97, 6.59), (13.32, 6.67), (14.79, 7.2), (15.99, 8.16), (16.49, 9.26)],
    ].map(points)

    /// The scribble in each petal: level strokes, each reaching nearly to the edge, turning round at the ends.
    private static let hatches: [[CGPoint]] = [
        [(7.83, 1.6), (10.64, 2.32), (7.02, 3.04), (11.12, 3.76), (6.71, 4.48), (11.16, 5.2), (6.82, 5.92), (10.82, 6.64), (7.63, 7.36)],
        [(4.3, 6.75), (7.85, 7.47), (2.14, 8.19), (7.74, 8.91), (1.81, 9.63), (6.29, 10.35)],
        [(10.53, 6.75), (14.92, 7.47), (10.06, 8.19), (15.98, 8.91), (11.01, 9.63), (16.29, 10.35)],
    ].map(points)

    private static let stem = points([(9, 10.4), (8.7, 13.6), (9.2, 17.4)])
    /// The leaf: one stroke out and back, low on the stem.
    private static let leaf = points([(9.2, 16.6), (11.4, 14.8), (13.6, 13.6), (12.6, 15.6), (9.5, 16.3)])

    private static func points(_ xy: [(CGFloat, CGFloat)]) -> [CGPoint] { xy.map { CGPoint(x: $0.0, y: $0.1) } }

    /// A Catmull-Rom curve through the points, as cubic Béziers.
    private static func smoothPath(_ p: [CGPoint], closed: Bool = false) -> CGPath {
        let path = CGMutablePath(), n = p.count
        guard n > 1 else { return path }
        func at(_ i: Int) -> CGPoint { closed ? p[(i % n + n) % n] : p[min(max(i, 0), n - 1)] }
        path.move(to: p[0])
        for i in 0..<(closed ? n : n - 1) {
            let p0 = at(i - 1), p1 = at(i), p2 = at(i + 1), p3 = at(i + 2)
            path.addCurve(to: p2,
                          control1: CGPoint(x: p1.x + (p2.x - p0.x) / 6, y: p1.y + (p2.y - p0.y) / 6),
                          control2: CGPoint(x: p2.x - (p3.x - p1.x) / 6, y: p2.y - (p3.y - p1.y) / 6))
        }
        if closed { path.closeSubpath() }
        return path
    }
}

/// The menu-bar mark drawn as a view, for snapshots: the bar's ink is given, not read.
struct MenuBarMark: View {
    var dark: Bool
    var body: some View {
        Canvas { context, size in
            context.withCGContext { ctx in
                ctx.scaleBy(x: size.width / MenuBarIcon.size.width, y: size.height / MenuBarIcon.size.height)
                MenuBarIcon.draw(in: ctx, ink: dark ? NSColor.white.cgColor : NSColor(white: 0, alpha: 0.85).cgColor)
            }
        }
    }
}
