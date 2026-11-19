import AppKit
import IrisCore
import SwiftUI

/* ─────────────────────────────────────────────────────────
 * ABOUT › "Where Iris comes from": the iris in its meadow
 *
 * A live wallpaper, not a film: one still scene that is quietly alive,
 * like a relaxing anime wallpaper. Nothing starts or ends; nothing asks
 * to be watched. Printed only in Iris's inks, like the coffee zine and
 * the landing page: the sky plate, the landing page's green plate for the
 * meadow and its paler green for the hill and the grass drawn over it, a
 * few blades in pink off register, the garden's pen-drawn sun, white
 * clouds, orange butterflies. No gradients, no glows: riso ink is flat
 * colour through a screen.
 *
 *   the wind     a slow breeze; gusts roll across the meadow left to
 *                right every few seconds (the grass ripples as one)
 *   depth        far grass small, still and printed toward the haze; near grass
 *                big, deep and swaying; the view drifts very slowly (70 s) so
 *                near layers slide more than far ones (parallax)
 *   grass        tall, irregular blades; each bends most at its tip,
 *                which trails the root, so gusts run up it and whip it
 *   the iris     sways on its stem with the same wind, a beat behind
 *   sun          the garden's pixel sun, rough brush loops spreading round it
 *                in a paler sky, breathing and turning slowly (the site's foot)
 *   clouds       the garden's clouds drift right, very slowly
 *   a butterfly  wanders round the flower, wings slow
 *   petals       a few pink petals float across on the breeze
 *
 * Like a summer anime still: the picture itself hardly moves. The breeze
 * is soft and rare, the view drifts barely at all; what's alive is the
 * light: warm beams from the sun and dust drifting slowly through them.
 *
 * Reduce Motion and snapshots: the same scene, still.
 * ───────────────────────────────────────────────────────── */

/// The wind and every pace in the scene. The only place speeds live.
private enum Breeze {
    static let gustEvery = 12.0       // seconds between gusts crossing the meadow
    static let gustSpeed = 70.0       // points a second a gust travels
    static let calm = 3.0             // degrees grass leans with no gust
    static let gust = 5.0             // degrees more at a gust's peak
    static let flutter = 0.7          // degrees of small, quick quiver
    static let tipLag = 0.35          // seconds a blade's tip trails its root, so it whips
    static let irisSway = 1.6         // degrees the iris leans at most
    static let frameRate = 30.0       // the scene never needs more
}

private enum Scene {
    static let size = CGSize(width: 612, height: 210)
    /// The far hill's top edge and the meadow's, as points across.
    static let hill: [CGPoint] = [CGPoint(x: -40, y: 108), CGPoint(x: 120, y: 82), CGPoint(x: 260, y: 94),
                                  CGPoint(x: 400, y: 74), CGPoint(x: 520, y: 88), CGPoint(x: 652, y: 76)]
    static let meadow: [CGPoint] = [CGPoint(x: -40, y: 136), CGPoint(x: 160, y: 126), CGPoint(x: 330, y: 132),
                                    CGPoint(x: 480, y: 122), CGPoint(x: 652, y: 127)]
    /// The air between here and the hills: far things are printed a step toward it.
    static let haze: RGB8 = (178, 216, 186)
    /// Two inks mixed, as a far thing printed a step toward the haze: one flat colour, never a gradient.
    static func mix(_ a: RGB8, _ b: RGB8, _ k: Double) -> Color {
        func c(_ x: UInt8, _ y: UInt8) -> Double { (Double(x) * (1 - k) + Double(y) * k) / 255 }
        return Color(red: c(a.0, b.0), green: c(a.1, b.1), blue: c(a.2, b.2))
    }
    static func mixRGB(_ a: RGB8, _ b: RGB8, _ k: Double) -> RGB8 {
        func c(_ x: UInt8, _ y: UInt8) -> UInt8 { UInt8(Double(x) * (1 - k) + Double(y) * k) }
        return (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
    }
    /// Parallax: how far each layer slides as the view drifts, by how near it is.
    static let drift = (period: 120.0, sky: 0.5 as CGFloat, hill: 1.5 as CGFloat, far: 2.5 as CGFloat, near: 6 as CGFloat)
    static let ink = Color(red: 42 / 255, green: 26 / 255, blue: 22 / 255)
    /// The landing page's green plate and the paler green drawn over it (iris-site, Desk.tsx WALL),
    /// and a deeper green for the nearest grass.
    static let plateGreen: RGB8 = (48, 146, 88)
    static let paleGreen: RGB8 = (86, 172, 116)
    static let deepGreen = Color(red: 28 / 255, green: 104 / 255, blue: 60 / 255)
    static let deepGreenRGB: RGB8 = (28, 104, 60)
    static let paleGreenInk = Color(red: 86 / 255, green: 172 / 255, blue: 116 / 255)
}

/// The garden's clouds, drifting right very slowly and coming round again.
/// The painted cumulus: puffs as shares of its width (x across from its left, y up from its
/// flat base, radius), three tones, and how slowly each drifts.
private enum Cumulus {
    struct Cloud { let x: CGFloat; let base: CGFloat; let width: CGFloat; let speed: CGFloat; let puffs: [(x: CGFloat, y: CGFloat, r: CGFloat)] }
    static let shade = Color(red: 150 / 255, green: 168 / 255, blue: 212 / 255)
    static let deep = Color(red: 128 / 255, green: 144 / 255, blue: 196 / 255)
    static let body = Color(red: 230 / 255, green: 236 / 255, blue: 248 / 255)
    static let light = Color(red: 255 / 255, green: 252 / 255, blue: 249 / 255)
    /// Grows a cumulus from many small puffs under a mound-shaped envelope: big round ones low
    /// down, small cauliflower bumps along the top edge, so its outline billows like a real one.
    static func grow(_ seed: Int, tall: CGFloat, count: Int) -> [(x: CGFloat, y: CGFloat, r: CGFloat)] {
        var out: [(x: CGFloat, y: CGFloat, r: CGFloat)] = []
        for k in 0..<count {
            let u = CGFloat(Curve.noise(seed * 101 + k * 7) * 0.5 + 0.5)
            let v = CGFloat(Curve.noise(seed * 37 + k * 13) * 0.5 + 0.5)
            // The mound: tallest a little left of the middle, rounded off at the ends.
            let top = tall * pow(sin(.pi * u), 0.7) * (1 - 0.25 * abs(u - 0.42))
            // Most puffs sit near the top edge, where the outline is.
            let y = top * (1 - pow(v, 2.2) * 0.85)
            let r = (0.03 + 0.06 * (1 - y / max(tall, 0.01)) + 0.025 * CGFloat(Curve.noise(seed + k * 3))) * (0.7 + tall)
            out.append((u, y, max(r, 0.02)))
        }
        return out
    }
    static let tower = grow(3, tall: 0.48, count: 90)
    static let bank = grow(9, tall: 0.26, count: 46)
    static let small = grow(14, tall: 0.24, count: 34)
    static let all: [Cloud] = [
        Cloud(x: 110, base: 116, width: 290, speed: 0, puffs: tower),
        Cloud(x: 380, base: 62, width: 120, speed: 0, puffs: bank),
        Cloud(x: -16, base: 52, width: 104, speed: 0, puffs: small),
    ]
}

private enum Clouds {
    struct Cloud { let kind: Sprites.CloudShape; let fill: RGB8; let x0: CGFloat; let y: CGFloat; let width: CGFloat; let speed: CGFloat; let seed: Int }
    static let all: [Cloud] = [
        Cloud(kind: .puffy, fill: Riso.paper, x0: 40, y: 30, width: 104, speed: 2.2, seed: 61),
        Cloud(kind: .long, fill: Riso.paper, x0: 250, y: 52, width: 120, speed: 3.0, seed: 62),
        Cloud(kind: .round, fill: Riso.paper, x0: 520, y: 26, width: 70, speed: 3.8, seed: 63),
    ]
    /// Soft and white: paper printed solid with only a pale sky edge, so the sky stays bright.
    @MainActor static let images: [NSImage] = all.map { Sprites.skyCloud($0.kind, $0.fill, density: 0, edge: Sprites.Blue.pale, seed: $0.seed) }
}

/// The meadow's grass: blades rooted along and below its top edge, each its own height.
private enum Grass {
    /// A blade: rooted, its own length and width, a lean and a kink of its own, so no two match.
    struct Blade { let root: CGPoint; let length: CGFloat; let width: CGFloat; let bend: CGFloat; let kink: CGFloat; let ink: Double; let bloom: Bloom?
        /// 0 at the meadow's far edge … 1 at the front: size, colour, sway and parallax follow it.
        let near: CGFloat }
    enum Bloom { case daisy, pink, yellow }
    static let blades: [Blade] = {
        var out: [Blade] = []
        let count = 520
        for i in 0..<count {
            let n1 = Curve.noise(i * 7 + 1), n2 = Curve.noise(i * 13 + 5), n3 = Curve.noise(i * 29 + 9), n4 = Curve.noise(i * 41 + 3)
            let x = CGFloat(Curve.noise(i * 53 + 11) * 0.5 + 0.5) * 700 - 44
            let top = edge(Scene.meadow, x)
            // In perspective: most blades far off, small, pale and still; fewer and fewer
            // nearer, each bigger, deeper and swaying more; the nearest run off the bottom.
            let u = CGFloat(abs(n2))
            let near = pow(u, 1.25)
            let depth = near * (Scene.size.height - top + 16)
            let tall: CGFloat = i % 7 == 0 ? 1.6 : 1
            let length = (13 + 9 * CGFloat(n3)) * (0.6 + 2.5 * near) * tall
            let bloom: Bloom? = i % 17 == 0 ? .daisy : (i % 25 == 0 ? .pink : (i % 29 == 0 ? .yellow : nil))
            out.append(Blade(root: CGPoint(x: x, y: top + depth + 2), length: length,
                             width: (0.8 + CGFloat(abs(n4)) * 1.0) * (0.55 + 2.2 * near), bend: CGFloat(n1) * 0.35,
                             kink: CGFloat(n4) * 0.5, ink: abs(n3), bloom: bloom, near: near))
        }
        // Back to front, so nearer grass draws over farther.
        out.sort { $0.root.y < $1.root.y }
        return out
    }()

    /// The height of an edge at x, eased between its points.
    static func edge(_ pts: [CGPoint], _ x: CGFloat) -> CGFloat {
        guard let j = pts.firstIndex(where: { $0.x >= x }), j > 0 else { return pts.first?.y ?? 0 }
        let a = pts[j - 1], b = pts[j]
        let k = (x - a.x) / max(b.x - a.x, 1)
        let s = k * k * (3 - 2 * k)
        return a.y + (b.y - a.y) * s
    }
}

private enum Iris {
    static let size: CGFloat = 112
    static let base = CGPoint(x: 438, y: 156)
}

/// The sun and the rings round it, as on the landing page's foot.
private enum SunSpot {
    static let at = CGPoint(x: 540, y: 52)
    static let rings = 7
    static let first: CGFloat = 40          // the smallest ring's radius
    static let gap: CGFloat = 32            // between rings
    static let width: CGFloat = 11           // the brush's width
    static let wobble: CGFloat = 7          // how far a loop strays from round
    static let ink = Color(red: 148 / 255, green: 186 / 255, blue: 236 / 255)   // the site's skyGhost
}

/// The soft-focus foreground: big leaves rising into the corners from below the frame.
private enum Foreground {
    static let blur: CGFloat = 6
    struct Leaf { let root: CGPoint; let length: CGFloat; let width: CGFloat; let lean: Double; let ink: RGB8 }
    // Darker and bigger than anything in the meadow, so they read as right in front:
    // a deep green near espresso, the brand pinks, a little rose.
    static let leaves: [Leaf] = [
        Leaf(root: CGPoint(x: 8, y: 250), length: 196, width: 38, lean: 0.22, ink: (22, 70, 46)),
        Leaf(root: CGPoint(x: 44, y: 256), length: 158, width: 34, lean: 0.52, ink: (236, 110, 176)),
        Leaf(root: CGPoint(x: -20, y: 244), length: 150, width: 32, lean: -0.08, ink: (40, 112, 70)),
        Leaf(root: CGPoint(x: 86, y: 262), length: 118, width: 28, lean: 0.8, ink: (170, 102, 124)),
        Leaf(root: CGPoint(x: 602, y: 254), length: 176, width: 36, lean: -0.3, ink: (22, 70, 46)),
        Leaf(root: CGPoint(x: 634, y: 244), length: 132, width: 30, lean: -0.08, ink: (240, 134, 176)),
        Leaf(root: CGPoint(x: 566, y: 262), length: 112, width: 26, lean: -0.62, ink: (40, 112, 70)),
    ]
}

private enum Butterfly {
    static let size: CGFloat = 22
    /// Each one's wander: where it circles, how far, how fast, and where it starts.
    static let all: [(centre: CGPoint, reach: CGSize, speed: Double, phase: Double)] = [
        (CGPoint(x: 438, y: 70), CGSize(width: 70, height: 20), 0.16, 0),
        (CGPoint(x: 200, y: 92), CGSize(width: 90, height: 18), 0.12, 2.2),
        (CGPoint(x: 330, y: 52), CGSize(width: 60, height: 16), 0.19, 4.1),
    ]
}

private enum Petals {
    static let count = 5
    static let speed: CGFloat = 7         // points a second, on the breeze
}

/// The iris in its meadow, quietly alive: About's picture of where Iris comes from.
struct MakersJourney: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.designState) private var design
    /// Stopped by the pause button (WCAG 2.2.2: motion that runs on needs a way to stop it). The
    /// story's phrases under the meadow stop with it, so one button stops everything that moves.
    @Binding var paused: Bool
    @State private var pausedAt: Double?

    private var still: Bool { reduceMotion || design != nil }

    var body: some View {
        // On the test display (`OnDisplay`) it holds still, so a capture is of one frame.
        TimelineView(.animation(minimumInterval: 1 / Breeze.frameRate, paused: still || pausedAt != nil || OnDisplay.isOn)) { context in
            let t = still || OnDisplay.isOn ? 3.3 : (pausedAt ?? context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 3600))
            ZStack(alignment: .topLeading) {
                ScreenPlate(ink: .sky, seed: 11)
                // The landing page's foot: rough brush loops in a paler sky spreading round the sun.
                let cam = CGFloat(sin(t * 2 * .pi / Scene.drift.period))
                Group {
                    Canvas { ctx, _ in drawRings(&ctx, t) }
                    sun(t)
                    clouds(t)
                }
                .offset(x: cam * Scene.drift.sky)
                // Far off and a touch out of focus.
                hill.offset(x: cam * Scene.drift.hill).blur(radius: 1.2)
                meadow.offset(x: cam * Scene.drift.far)
                Canvas { ctx, _ in drawGrass(&ctx, t, behind: true) }
                flower(t)
                Canvas { ctx, _ in drawGrass(&ctx, t, behind: false); drawPetals(&ctx, t) }
                ForEach(Butterfly.all.indices, id: \.self) { butterfly($0, t) }
                // The light, as in a summer anime still: warm beams falling from the sun across
                // the meadow, and dust drifting slowly through them. Almost nothing else moves.
                Canvas { ctx, _ in drawBeams(&ctx, t); drawDust(&ctx, t) }
                    .allowsHitTesting(false)
                // Right in front of the lens: big leaves in the corners, soft and grainy, and a
                // butterfly drifting past out of focus. They slide the most as the view drifts.
                Canvas { ctx, _ in drawForeground(&ctx, t) }
                    .offset(x: cam * Scene.drift.near * 1.6)
                    .blur(radius: Foreground.blur)
                    .grain(0.45)
                closeButterfly(t).offset(x: cam * Scene.drift.near * 1.6)
            }
            .frame(width: Scene.size.width, height: Scene.size.height, alignment: .topLeading)
        }
        .frame(width: Scene.size.width, height: Scene.size.height, alignment: .topLeading)
        // Torn from a sheet of paper: a deckled edge, no drawn frame.
        .clipShape(Deckle(seed: 7))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("The iris in a meadow, swaying in a breeze.")
        .accessibilityAddTraits(.isImage)
        .overlay(alignment: .bottomTrailing) { if !still { pauseButton.padding(10) } }
    }

    /// Stops the meadow, or starts it again: a small paper disc with a pen pause or play mark.
    private var pauseButton: some View {
        Button {
            pausedAt = pausedAt == nil ? Date().timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 3600) : nil
            paused = pausedAt != nil
        } label: {
            ZStack {
                Circle().fill(DS.Surface.top.opacity(0.92)).frame(width: 26, height: 26)
                    .overlay(HandEllipse(seed: 21, wobble: 0.4).stroke(DS.Ink.i12.opacity(0.55), lineWidth: 1.1))
                if pausedAt == nil {
                    HStack(spacing: 3) {
                        Capsule().fill(DS.Ink.i12).frame(width: 2.6, height: 10)
                        Capsule().fill(DS.Ink.i12).frame(width: 2.6, height: 10)
                    }
                } else {
                    Path { p in p.move(to: CGPoint(x: 0, y: 0)); p.addLine(to: CGPoint(x: 9, y: 5.5)); p.addLine(to: CGPoint(x: 0, y: 11)); p.closeSubpath() }
                        .fill(DS.Ink.i12).frame(width: 9, height: 11).offset(x: 1)
                }
            }
            .contentShape(Circle())
        }
        .buttonStyle(SqueezeStyle(scale: 0.96))
        .help(pausedAt == nil ? "Pause the animation" : "Play the animation")
        .accessibilityLabel(pausedAt == nil ? "Pause the animation" : "Play the animation")
    }

    // MARK: The wind

    /// How hard the wind blows at x: a calm lean, a gust rolling across every few seconds,
    /// and a small quick quiver. Degrees.
    private func wind(_ x: CGFloat, _ t: Double) -> Double {
        let travel = Breeze.gustEvery * Breeze.gustSpeed
        let front = (t * Breeze.gustSpeed).truncatingRemainder(dividingBy: travel) - 200
        let d = (Double(x) - front) / 160
        let gust = exp(-d * d)
        let quiver = sin(t * 3.1 + Double(x) * 0.07) * Breeze.flutter
        return Breeze.calm + Breeze.gust * gust + quiver
    }

    // MARK: Land

    private var hill: some View {
        edgeShape(Scene.hill)
            .printedInk(Scene.mixRGB(Scene.paleGreen, Scene.haze, 0.3), seed: 21)
            .overlay(edgeLine(Scene.hill).stroke(Scene.ink.opacity(0.5), style: StrokeStyle(lineWidth: 1.4, lineCap: .round)))
    }

    private var meadow: some View {
        edgeShape(Scene.meadow)
            .printedInk(Scene.plateGreen, seed: 22)
            .overlay(edgeLine(Scene.meadow).stroke(Scene.ink.opacity(0.85), style: StrokeStyle(lineWidth: 1.8, lineCap: .round)))
    }

    private func edgeShape(_ pts: [CGPoint]) -> some Shape {
        EdgeShape(points: pts, closed: true)
    }

    private func edgeLine(_ pts: [CGPoint]) -> some Shape {
        EdgeShape(points: pts, closed: false)
    }

    // MARK: Life

    /// Clouds painted like a summer anime sky: big billowing cumulus with no outline, each
    /// built from three flat tones: a blue-lavender shade underneath, a soft white body, and
    /// bright paper where the sun (to the right) catches the tops. A flat base, as cumulus has.
    /// They drift very slowly and keep their shapes.
    private func clouds(_ t: Double) -> some View {
        Canvas { ctx, size in
            for (i, c) in Cumulus.all.enumerated() {
                // Almost still, as in the reference: a few points' drift over minutes.
                let x = c.x + CGFloat(sin(t * 2 * .pi / 400 + Double(i) * 2)) * 10
                drawCumulus(&ctx, c, at: CGPoint(x: x, y: c.base))
            }
        }
        // Soft edges, painted, not drawn: the tones melt into each other.
        .blur(radius: 0.9)
    }

    private func drawCumulus(_ ctx: inout GraphicsContext, _ c: Cumulus.Cloud, at o: CGPoint) {
        let puffs = c.puffs.map { (p: CGPoint(x: o.x + $0.x * c.width, y: o.y - $0.y * c.width), r: $0.r * c.width) }
        let floor = Path(CGRect(x: o.x - c.width, y: o.y - c.width * 2, width: c.width * 3, height: c.width * 2))
        ctx.drawLayer { l in
            // Everything above the flat base.
            l.clip(to: floor)
            func fill(_ ink: Color, dx: CGFloat, dy: CGFloat, scale: CGFloat) {
                var path = Path()
                for p in puffs {
                    let r = p.r * scale
                    path.addEllipse(in: CGRect(x: p.p.x + dx * p.r - r, y: p.p.y + dy * p.r - r, width: r * 2, height: r * 2))
                }
                l.fill(path, with: .color(ink))
            }
            fill(Cumulus.deep, dx: -0.1, dy: 0.25, scale: 1.0)       // the deepest shade, the bellies
            fill(Cumulus.shade, dx: 0, dy: 0.05, scale: 0.98)        // the shade round the back
            fill(Cumulus.body, dx: 0.18, dy: -0.22, scale: 0.84)     // the soft white body, lifted toward the light
            fill(Cumulus.light, dx: 0.3, dy: -0.38, scale: 0.62)     // where the sun catches the tops
        }
    }

    /// The grass in two layers, so the iris stands among it: blades behind its stem, then in front.
    private func drawGrass(_ ctx: inout GraphicsContext, _ t: Double, behind: Bool) {
        // Behind the iris: the grass rooted above its foot; in front: the rest.
        let cam = CGFloat(sin(t * 2 * .pi / Scene.drift.period))
        for (i, b) in Grass.blades.enumerated() where (b.root.y < Iris.base.y - 4) == behind {
            let slide = cam * (Scene.drift.far + (Scene.drift.near - Scene.drift.far) * b.near)
            let sway = Double(0.45 + 0.75 * b.near)
            // The blade bends more toward its tip, and the tip trails the root, so a gust
            // runs up each blade and whips it.
            let steps = 6
            var spine: [CGPoint] = [CGPoint(x: b.root.x + slide, y: b.root.y)]
            var angle = 0.0
            for k in 1...steps {
                let f = Double(k) / Double(steps)
                let w = wind(b.root.x, t - Breeze.tipLag * f) * Double.pi / 180
                angle = Double(b.bend) + (w * 1.4 * sway + Double(b.kink) * 0.6) * pow(f, 1.5)
                let seg = b.length / CGFloat(steps)
                let last = spine[spine.count - 1]
                spine.append(CGPoint(x: last.x + CGFloat(sin(angle)) * seg, y: last.y - CGFloat(cos(angle)) * seg))
            }
            var left: [CGPoint] = [], right: [CGPoint] = []
            for (k, q) in spine.enumerated() {
                let half = b.width / 2 * (1 - CGFloat(k) / CGFloat(steps)) + 0.15
                left.append(CGPoint(x: q.x - half, y: q.y)); right.append(CGPoint(x: q.x + half, y: q.y))
            }
            var p = Path()
            p.move(to: left[0])
            for k in 1..<left.count { p.addQuadCurve(to: CGPoint(x: (left[k - 1].x + left[k].x) / 2, y: (left[k - 1].y + left[k].y) / 2), control: left[k - 1]) }
            p.addLine(to: spine[steps])
            for k in stride(from: right.count - 1, through: 1, by: -1) {
                p.addQuadCurve(to: CGPoint(x: (right[k - 1].x + right[k].x) / 2, y: (right[k - 1].y + right[k].y) / 2), control: right[k])
            }
            p.addLine(to: right[0])
            p.closeSubpath()
            // Far blades are printed a step toward the haze; near ones in full ink. Some near
            // ones pink with their ink line landed a little off it.
            let far = Double(1 - b.near)
            if i % 13 == 0 && b.near > 0.35 {
                ctx.fill(p.offsetBy(dx: 1.6, dy: 1.2), with: .color(DS.color(Riso.pink)))
                ctx.stroke(p, with: .color(Scene.ink.opacity(0.75)), lineWidth: 0.8)
            } else {
                let base: RGB8 = b.ink > 0.45 ? Scene.deepGreenRGB : Scene.paleGreen
                ctx.fill(p, with: .color(Scene.mix(base, Scene.haze, 0.62 * far * far)))
            }
            if let bloom = b.bloom { drawBloom(&ctx, bloom, at: spine[steps], t: t, seed: i, scale: 0.45 + 0.9 * b.near) }
        }
    }

    /// A small flower on a blade's tip, nodding with it: a daisy, a pink cup or a buttercup.
    private func drawBloom(_ ctx: inout GraphicsContext, _ kind: Grass.Bloom, at c: CGPoint, t: Double, seed: Int, scale: CGFloat = 1) {
        ctx.drawLayer { l in
            l.translateBy(x: c.x, y: c.y)
            l.scaleBy(x: scale, y: scale)
            l.rotate(by: .degrees(sin(t * 1.4 + Double(seed)) * 8))
            switch kind {
            case .daisy:
                for k in 0..<6 {
                    l.drawLayer { p in
                        p.rotate(by: .degrees(Double(k) * 60))
                        let petal = Path(ellipseIn: CGRect(x: -1.6, y: -6, width: 3.2, height: 5.4))
                        p.fill(petal, with: .color(DS.Surface.top))
                        p.stroke(petal, with: .color(Scene.ink.opacity(0.8)), lineWidth: 0.6)
                    }
                }
                l.fill(Path(ellipseIn: CGRect(x: -2, y: -2, width: 4, height: 4)), with: .color(DS.color(Riso.sunflower)))
            case .pink:
                l.fill(Path(ellipseIn: CGRect(x: -3.6, y: -3.6, width: 7.2, height: 7.2)), with: .color(DS.color(Riso.platePink)))
                l.fill(Path(ellipseIn: CGRect(x: -1.2, y: -1.2, width: 2.4, height: 2.4)), with: .color(.white.opacity(0.9)))
            case .yellow:
                l.fill(Path(ellipseIn: CGRect(x: -3, y: -3, width: 6, height: 6)), with: .color(DS.color(Riso.sunflower)))
            }
        }
    }

    /// Each ring a loop drawn by hand: its radius strays as it goes round and overshoots where
    /// it closes; the whole set breathes and turns very slowly.
    private func drawRings(_ ctx: inout GraphicsContext, _ t: Double) {
        let c = SunSpot.at
        for k in 0..<SunSpot.rings {
            let r = SunSpot.first + CGFloat(k) * SunSpot.gap + CGFloat(sin(t * 0.5 - Double(k) * 0.6)) * 3
            let turn = t * 0.04 * (k % 2 == 0 ? 1 : -1) + Double(k)
            var p = Path()
            let n = 90
            for j in 0...(n + 6) {
                let a = Double(j) / Double(n) * 2 * .pi + turn
                let stray = SunSpot.wobble * CGFloat(Curve.noise(k * 97 + j % n) * 0.6 + sin(a * 3 + Double(k)) * 0.5)
                let q = CGPoint(x: c.x + (r + stray) * CGFloat(cos(a)), y: c.y + (r + stray) * CGFloat(sin(a)) * 0.92)
                if j == 0 { p.move(to: q) } else { p.addLine(to: q) }
            }
            ctx.stroke(p, with: .color(SunSpot.ink.opacity(1 - Double(k) * 0.05)),
                       style: StrokeStyle(lineWidth: SunSpot.width * (1 - CGFloat(k) * 0.05), lineCap: .round, lineJoin: .round))
        }
    }

    private func sun(_ t: Double) -> some View {
        let at = SunSpot.at
        // The garden's sun, the one on setup's plate and the landing page: drawn in pen,
        // its rays turning very slowly. No glow: riso ink doesn't glow.
        return Image(nsImage: Sprites.sun).interpolation(.none).resizable()
            .frame(width: 46, height: 46)
            .rotationEffect(.degrees(t * 3))
            .position(at)
    }

    /// Sunbeams: a few long bands of warm paper falling from the sun down to the left,
    /// flat and faint (printed, not airbrushed), each breathing very slowly.
    private func drawBeams(_ ctx: inout GraphicsContext, _ t: Double) {
        let from = SunSpot.at
        let beams: [(angle: Double, spread: Double, alpha: Double)] = [(2.28, 0.07, 0.22), (2.47, 0.045, 0.17), (2.64, 0.08, 0.2), (2.84, 0.04, 0.14)]
        for (k, b) in beams.enumerated() {
            let breathe = 0.65 + 0.35 * sin(t * 0.18 + Double(k) * 1.7)
            var p = Path()
            p.move(to: from)
            p.addLine(to: CGPoint(x: from.x + CGFloat(cos(b.angle - b.spread)) * 900, y: from.y + CGFloat(sin(b.angle - b.spread)) * 900))
            p.addLine(to: CGPoint(x: from.x + CGFloat(cos(b.angle + b.spread)) * 900, y: from.y + CGFloat(sin(b.angle + b.spread)) * 900))
            p.closeSubpath()
            ctx.fill(p, with: .color(Color(red: 1, green: 0.95, blue: 0.80).opacity(b.alpha * breathe)))
        }
    }

    /// Dust in the light: soft specks rising and drifting very slowly, each its own size and
    /// pace, twinkling as they turn.
    private func drawDust(_ ctx: inout GraphicsContext, _ t: Double) {
        let h = Double(Scene.size.height + 20)
        for i in 0..<34 {
            let n1 = Curve.noise(i * 17 + 3), n2 = Curve.noise(i * 31 + 7), n3 = Curve.noise(i * 43 + 1)
            let speed = 2.5 + 3 * abs(n1)
            let y = h - (Double(i) * 37 + t * speed).truncatingRemainder(dividingBy: h) - 10
            let x = Double(abs(n2)) * Double(Scene.size.width) + sin(t * 0.25 + Double(i)) * 14
            let r = 0.8 + 2.2 * abs(n3)
            let twinkle = 0.35 + 0.65 * abs(sin(t * (0.4 + abs(n1)) + Double(i)))
            ctx.fill(Path(ellipseIn: CGRect(x: x - r, y: y - r, width: r * 2, height: r * 2)),
                     with: .color(Color(red: 1, green: 0.97, blue: 0.88).opacity(0.75 * twinkle)))
        }
    }

    /// Big leaves right in front, swaying slowly and a little behind the breeze.
    private func drawForeground(_ ctx: inout GraphicsContext, _ t: Double) {
        for (i, l) in Foreground.leaves.enumerated() {
            let w = (wind(l.root.x, t - 0.6) - Breeze.calm) * Double.pi / 180 * 0.35 + sin(t * 0.5 + Double(i)) * 0.03
            let a = l.lean + w
            let tip = CGPoint(x: l.root.x + CGFloat(sin(a)) * l.length, y: l.root.y - CGFloat(cos(a)) * l.length)
            let mid = CGPoint(x: (l.root.x + tip.x) / 2, y: (l.root.y + tip.y) / 2)
            let nx = CGFloat(cos(a)) * l.width, ny = CGFloat(sin(a)) * l.width
            var p = Path()
            p.move(to: l.root)
            p.addQuadCurve(to: tip, control: CGPoint(x: mid.x - nx, y: mid.y - ny))
            p.addQuadCurve(to: l.root, control: CGPoint(x: mid.x + nx, y: mid.y + ny))
            ctx.fill(p, with: .color(Scene.mix(l.ink, l.ink, 0)))
        }
    }

    /// One butterfly close to the lens: big, out of focus, drifting slowly across.
    private func closeButterfly(_ t: Double) -> some View {
        let span = Double(Scene.size.width + 120)
        let x = (t * 4).truncatingRemainder(dividingBy: span) - 60
        let y = 150 + sin(t * 0.7) * 22
        return Image(nsImage: Sprites.butterfly).interpolation(.none).resizable()
            .hueRotation(.degrees(-48))
            .frame(width: 40, height: 40)
            .scaleEffect(x: 0.5 + 0.5 * abs(sin(t * 2.2)), y: 1)
            .rotationEffect(.degrees(sin(t * 0.9) * 14))
            .blur(radius: 3)
            .position(x: x, y: y)
            .allowsHitTesting(false)
    }

    /// A few petals carried across on the breeze, turning slowly.
    private func drawPetals(_ ctx: inout GraphicsContext, _ t: Double) {
        let span = Double(Scene.size.width + 60)
        for i in 0..<Petals.count {
            let x = (Double(i) * span / Double(Petals.count) + t * Double(Petals.speed) * (0.8 + 0.1 * Double(i))).truncatingRemainder(dividingBy: span) - 30
            let y = 70 + Double(i * 23 % 70) + sin(t * 0.6 + Double(i) * 1.7) * 12
            ctx.drawLayer { l in
                l.translateBy(x: x, y: y)
                l.rotate(by: .degrees(t * 40 + Double(i) * 70))
                l.scaleBy(x: CGFloat(0.5 + 0.5 * abs(sin(t * 0.9 + Double(i)))), y: 1)
                l.fill(Path(ellipseIn: CGRect(x: -2.5, y: -4, width: 5, height: 8)), with: .color(DS.color(Riso.pink).opacity(0.9)))
            }
        }
    }

    private func flower(_ t: Double) -> some View {
        let k = Iris.size / 92
        // The flower open and at rest; the wind leans it, a beat behind the grass at its foot.
        let clock = RigClock(now: 100, arrived: 0, bloomed: 1, full: false)
        let sway = (wind(Iris.base.x, t - 0.4) - Breeze.calm) / Breeze.gust * Breeze.irisSway + sin(t * 0.8) * 0.6
        return FlowerRig(clock: clock, size: Iris.size)
            .rotationEffect(.degrees(sway), anchor: UnitPoint(x: IrisRig.base.x / IrisRig.canvas.width, y: IrisRig.base.y / IrisRig.canvas.height))
            // It stands a third of the way into the meadow, so it slides with the grass there.
            .offset(x: CGFloat(sin(t * 2 * .pi / Scene.drift.period)) * (Scene.drift.far + (Scene.drift.near - Scene.drift.far) * 0.3))
            .position(x: Iris.base.x - IrisRig.base.x * k + IrisRig.canvas.width * k / 2,
                      y: Iris.base.y - IrisRig.base.y * k + IrisRig.canvas.height * k / 2)
            .allowsHitTesting(false)
    }

    /// One butterfly wandering round the flower on a slow figure of eight, wings slow.
    private func butterfly(_ i: Int, _ t: Double) -> some View {
        let b = Butterfly.all[i]
        let a = t * b.speed + b.phase
        let p = CGPoint(x: b.centre.x + b.reach.width * CGFloat(sin(a)),
                        y: b.centre.y + b.reach.height * CGFloat(sin(a * 2)) - 6 * CGFloat(sin(t * 1.3 + b.phase)))
        // Three kinds, as on the landing page's foot: the garden's orange, a rose and a pale one.
        let kind = i % 3
        return Image(nsImage: Sprites.butterfly).interpolation(.none).resizable()
            .hueRotation(.degrees(kind == 1 ? -48 : 0))
            .saturation(kind == 2 ? 0.15 : 1)
            .brightness(kind == 2 ? 0.25 : 0)
            .frame(width: Butterfly.size * (kind == 0 ? 1 : 0.8), height: Butterfly.size * (kind == 0 ? 1 : 0.8))
            .scaleEffect(x: 0.4 + 0.6 * abs(sin(t * 3 + b.phase)), y: 1)
            .rotationEffect(.degrees(cos(a) * 20 + (cos(a) < 0 ? 180 : 0)), anchor: .center)
            .scaleEffect(x: 1, y: cos(a) < 0 ? -1 : 1)
            .position(p)
            .allowsHitTesting(false)
    }
}

/// A smooth edge through points, closed down to the bottom (a hill) or left open (its line).
private struct EdgeShape: Shape {
    let points: [CGPoint]
    let closed: Bool

    func path(in rect: CGRect) -> Path {
        var p = Path()
        guard let first = points.first else { return p }
        p.move(to: first)
        for j in 1..<points.count {
            let a = points[j - 1], b = points[j]
            let mx = (a.x + b.x) / 2
            p.addCurve(to: b, control1: CGPoint(x: mx, y: a.y), control2: CGPoint(x: mx, y: b.y))
        }
        if closed {
            p.addLine(to: CGPoint(x: max(rect.maxX, points[points.count - 1].x), y: rect.maxY))
            p.addLine(to: CGPoint(x: min(rect.minX, first.x), y: rect.maxY))
            p.closeSubpath()
        }
        return p
    }
}

/// A torn paper edge: the sheet's border wanders in and out in small ragged steps.
private struct Deckle: Shape {
    let seed: Int
    func path(in r: CGRect) -> Path {
        var rng = SeededRandom(seed: seed)
        var p = Path()
        let step: CGFloat = 3
        func jag() -> CGFloat { CGFloat(rng.next()) * 4.5 }
        p.move(to: CGPoint(x: r.minX + jag(), y: r.minY + jag()))
        var x = r.minX
        while x < r.maxX { x += step; p.addLine(to: CGPoint(x: min(x, r.maxX), y: r.minY + jag())) }
        var y = r.minY
        while y < r.maxY { y += step; p.addLine(to: CGPoint(x: r.maxX - jag(), y: min(y, r.maxY))) }
        x = r.maxX
        while x > r.minX { x -= step; p.addLine(to: CGPoint(x: max(x, r.minX), y: r.maxY - jag())) }
        y = r.maxY
        while y > r.minY { y -= step; p.addLine(to: CGPoint(x: r.minX + jag(), y: max(y, r.minY))) }
        p.closeSubpath()
        return p
    }
}
