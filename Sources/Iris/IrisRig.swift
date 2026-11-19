import AppKit
import IrisCore
import SwiftUI

// The iris as a rig of parts drawn like the zine's brewers: a heavy ink line that boils a
// little, with the colour printed off register in a coarse riso screen of dots, like the logo's
// flower, each petal shading from a pale tip to a deep heart through a ramp of pinks. Each part
// acts on its own: the brown ground, the stem, the sword leaf, the bud, five standards fanned
// over the falls, a level pair between, three bearded falls and a dotted heart. Nothing is ever see-through (riso ink isn't): parts come
// and go by growing and shrinking. Every pose is worked out from the time since each beat, so
// the same moment always looks the same (and renders off screen, frame by frame).

/// Where the flower is in its life, as times since each beat (nil: hasn't happened).
struct RigClock {
    var now: Double
    var arrived: Double?
    var bloomed: Double?
    var wilted: Double?
    /// The full show (drop, splash, growth, the petal's flight) or the quick one, once it's familiar.
    var full = true
    /// Whether it went because the fix was put in (a stamp with a tick) or was dismissed (it just goes).
    var replaced = true

    func since(_ t: Double?) -> Double? { t.map { now - $0 } }
}

/// Motion curves for the rig.
enum Curve {
    static func clamp(_ x: Double) -> Double { min(1, max(0, x)) }
    static func easeOut(_ x: Double) -> Double { let c = clamp(x); return 1 - pow(1 - c, 3) }
    static func easeIn(_ x: Double) -> Double { let c = clamp(x); return c * c * c }
    static func easeInOut(_ x: Double) -> Double { let c = clamp(x); return c < 0.5 ? 4 * c * c * c : 1 - pow(-2 * c + 2, 3) / 2 }
    /// 0 → 1 over about `duration`, overshooting by `bounce` (0 none … 1 lots).
    static func spring(_ t: Double, _ duration: Double, bounce: Double = 0.4) -> Double {
        guard t > 0 else { return 0 }
        let x = t / duration
        let damping = 5.5 * (1 - bounce * 0.7)
        return 1 - exp(-x * damping) * cos(x * .pi * (1.4 + bounce * 1.2))
    }
    /// A bump 0 → 1 → 0 over `duration`, peaking a third of the way.
    static func bump(_ t: Double, _ duration: Double) -> Double {
        guard t > 0, t < duration else { return 0 }
        let x = t / duration
        return x < 0.33 ? easeOut(x / 0.33) : 1 - easeIn((x - 0.33) / 0.67)
    }
    /// A steady pseudo-random number in -1…1 for a seed.
    static func noise(_ seed: Int) -> Double {
        var x = UInt64(bitPattern: Int64(seed)) &* 0x9E37_79B9_7F4A_7C15
        x ^= x >> 29; x = x &* 0xBF58_476D_1CE4_E5B9; x ^= x >> 32
        return Double(x % 20001) / 10000 - 1
    }
}

/// The iris, its ink drop and its pop, posed for one moment. The flower's base sits at
/// `IrisRig.base` in an `IrisRig.canvas` frame (scaled by `size / 92`).
struct FlowerRig: View {
    let clock: RigClock
    var size: CGFloat = 92

    var body: some View {
        let k = size / 92
        let pose = IrisPose(clock)
        Canvas { ctx, _ in
            IrisRig.draw(pose, in: &ctx, k: k)
        }
        .frame(width: IrisRig.canvas.width * k, height: IrisRig.canvas.height * k)
        .accessibilityHidden(true)
    }
}

enum IrisRig {
    /// The drawing area, in points at size 92; the flower's foot stands at `base`.
    static let canvas = CGSize(width: 180, height: 200)
    static let base = CGPoint(x: 90, y: 166)
    /// The stem's height: the head (and the panel's corner) sits this far above the base.
    static let stem: CGFloat = 70
    /// How far the open bloom reaches left and right of the base, and up from it, swaying
    /// and recoiling included.
    static let reach = (left: CGFloat(37), right: CGFloat(42), height: CGFloat(100))

    static let ink = Color(red: 38 / 255, green: 24 / 255, blue: 22 / 255)
    static let pink = Color(red: 233 / 255, green: 107 / 255, blue: 192 / 255)
    static let paper = Color(red: 252 / 255, green: 248 / 255, blue: 240 / 255)
    @MainActor static let grain = Image(nsImage: Grain.tile)
    @MainActor static let salt = Image(nsImage: Grain.salt)
    /// An ink, 0–255 a channel.
    struct RGB {
        let r: CGFloat, g: CGFloat, b: CGFloat
        init(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat) { self.r = r; self.g = g; self.b = b }
        init(_ ink: RGB8) { self.init(CGFloat(ink.0), CGFloat(ink.1), CGFloat(ink.2)) }
    }
    /// The flower's inks: the brand's pinks and a step either side, a lilac only an iris gets,
    /// and the design system's browns for the ground.
    enum Inks {
        static let blush = RGB(250, 196, 218)      // a step paler than the brand pink
        static let pink = RGB(Riso.pink)
        static let hot = RGB(233, 107, 192)        // the rig's print pink
        static let magenta = RGB(208, 64, 150)     // a step deeper, a little off the system
        static let raspberry = RGB(Riso.pinkText)
        static let lilac = RGB(200, 160, 226)      // off the system: the outer standards' tips
        static let tan = RGB(Riso.tan)
        static let espresso = RGB(Riso.espresso)
    }

    /// The logo's print (`RisoArt.print`): an ink as ordered-dither dots, a 4×4 Bayer screen with
    /// a little noise, from none (0) to solid (1) in 16 steps, a dot a point. Every tile uses the
    /// same noisy field, so a deeper tone keeps a paler one's dots and adds to them.
    @MainActor private static var dotTiles: [Int: Image] = [:]
    @MainActor static func dots(_ c: RGB, _ density: CGFloat) -> Image {
        let level = Int((min(1, max(0, density)) * 16).rounded())
        let key = (Int(c.r) << 24) | (Int(c.g) << 16) | (Int(c.b) << 8) | level
        if let hit = dotTiles[key] { return hit }
        let tile = Image(nsImage: screen(CGFloat(level) / 16, c))
        dotTiles[key] = tile
        return tile
    }

    static func screen(_ density: CGFloat, _ c: RGB) -> NSImage {
        let cells = 16, px = 4, w = cells * px
        var rng = SeededRandom(seed: 11)
        var out = [UInt8](repeating: 0, count: w * w * 4)
        for cy in 0..<cells {
            for cx in 0..<cells {
                let t = (RisoArt.bayer[cy % 4][cx % 4] + 0.5) / 16 + (rng.next() - 0.5) * 0.3
                guard density > t else { continue }
                for y in cy * px..<(cy + 1) * px {
                    for x in cx * px..<(cx + 1) * px {
                        let o = (y * w + x) * 4
                        out[o] = UInt8(c.r); out[o + 1] = UInt8(c.g); out[o + 2] = UInt8(c.b); out[o + 3] = 255
                    }
                }
            }
        }
        return RisoArt.image(rgba: out, width: w, height: w, points: NSSize(width: cells, height: cells))
    }

    /// A tone for a part printed in dots: light at its tip, deeper toward its base, the ink
    /// shifting along `ramp` (tip first) as it goes.
    struct Tone { let tip: CGFloat; let base: CGFloat; var ramp: [RGB] = [Inks.hot] }
    static let soft = [Inks.blush, Inks.pink, Inks.hot, Inks.magenta]
    static let deep = [Inks.pink, Inks.hot, Inks.magenta, Inks.raspberry]
    static let outer = [Inks.lilac, Inks.blush, Inks.pink, Inks.hot]
    static let heartInks = [Inks.magenta, Inks.raspberry]
    static let budInks = [Inks.hot, Inks.magenta, Inks.raspberry]
    /// How big a dot is, in points: a coarse screen, like a riso poster seen up close.
    static let dotScale: CGFloat = 1.6
    /// The ground: tan at the edges, espresso where the stem comes out.
    static let soil = Tone(tip: 0.85, base: 1, ramp: [Inks.tan, Inks.tan, Inks.espresso])

    /// A fall behind the head that opens with the petal it follows.
    struct Extra { let follow: Int; let start: Double; let open: Double; let length: CGFloat; let width: CGFloat; let tone: Tone }
    static let extras: [Extra] = [
        Extra(follow: 3, start: -84, open: -96, length: 31, width: 10, tone: Tone(tip: 0.45, base: 0.85, ramp: soft)),
        Extra(follow: 4, start: 84, open: 96, length: 31, width: 10, tone: Tone(tip: 0.45, base: 0.85, ramp: soft)),
    ]

    /// Where the flicked fall lands as a stamp: above the text box's top edge, just in from
    /// its corner, clear of the plant and of the text.
    static let stampAt = CGPoint(x: reach.right + 22, y: -19)

    // MARK: Drawing

    @MainActor static func draw(_ p: IrisPose, in ctx: inout GraphicsContext, k: CGFloat) {
        ctx.scaleBy(x: k, y: k)
        let b = base
        let pen = Pen(seed: p.boil)

        // The puddle the drop spilled, and the specks it threw.
        if p.puddle > 0.01 {
            let w = 34 * p.puddle, h = 6 * p.puddle
            var puddle = Path(ellipseIn: CGRect(x: -w / 2, y: -h / 2, width: w, height: h))
            puddle.addEllipse(in: CGRect(x: w * 0.38, y: -h * 0.15, width: 5 * p.puddle, height: 3 * p.puddle))
            var g = ctx
            g.translateBy(x: b.x, y: b.y)
            fillScreen(puddle, reg: .zero, tone: soil, in: &g)
        }
        for (i, fleck) in p.flecks.enumerated() where fleck.size > 0.2 {
            var g = ctx
            g.translateBy(x: b.x + fleck.at.x, y: b.y + fleck.at.y)
            fillScreen(blob(at: .zero, size: fleck.size, seed: i), reg: .zero, tone: soil, in: &g)
        }

        // The plant, leaning and ducking about its base; when it sinks it goes into the puddle.
        var plant = ctx
        if p.sink > 0 { plant.clip(to: Path(CGRect(x: 0, y: 0, width: canvas.width, height: b.y + 1))) }
        plant.translateBy(x: b.x, y: b.y + p.sink)
        plant.rotate(by: .degrees(p.lean))
        plant.scaleBy(x: p.squashX, y: p.squashY)

        // The sword leaf, on the far side of the stem from your text so they never cross.
        if p.leaf > 0.01 {
            var leaf = plant
            leaf.translateBy(x: -3, y: 0)
            leaf.rotate(by: .degrees(p.leafTurn - 10))
            leaf.scaleBy(x: 1, y: p.leaf)
            var blade = Path()
            blade.move(to: .zero)
            blade.addQuadCurve(to: CGPoint(x: -13, y: -58), control: CGPoint(x: 1, y: -33))
            blade.addQuadCurve(to: CGPoint(x: -7, y: 0), control: CGPoint(x: -13, y: -22))
            blade.closeSubpath()
            leaf.fill(blade, with: .color(paper))
            pen.stroke(blade, width: 2.2, in: &leaf)
            var vein = Path()
            vein.move(to: CGPoint(x: -3.5, y: -3))
            vein.addQuadCurve(to: CGPoint(x: -11.5, y: -44), control: CGPoint(x: -4, y: -28))
            pen.stroke(vein, width: 0.9, in: &leaf)
        }

        // The stem: thick at the ground, thin at the head, curving with the sway.
        let top = CGPoint(x: p.curl * 0.5, y: -stem * p.growth * p.stretch)
        if p.growth > 0.01 {
            let c1 = CGPoint(x: -7 * p.bend, y: top.y * 0.4)
            let c2 = CGPoint(x: 6 * p.bend + p.curl * 0.2, y: top.y * 0.75)
            pen.taper(from: .zero, c1, c2, top, widths: (5.6, 3.2), in: &plant)
            pen.taper(from: .zero, c1, c2, top, widths: (2.4, 1.0), color: pink, in: &plant)
        }

        // The head, riding the stem's tip.
        if p.headShown {
            var head = plant
            head.translateBy(x: top.x, y: top.y)
            head.rotate(by: .degrees(p.headTurn))
            head.scaleBy(x: p.headX * p.bud, y: p.headY * p.bud)
            drawHead(p, pen: pen, in: &head)
        }

        // The pop when it opens: grained blots and pen flicks, out to the left, away from the panel.
        let crown = CGPoint(x: b.x, y: b.y - stem)
        for (i, dot) in p.dots.enumerated() where dot.size > 0.2 {
            fillPink(blob(at: CGPoint(x: crown.x + dot.at.x, y: crown.y + dot.at.y), size: dot.size, seed: 40 + i), reg: .zero, in: &ctx)
        }
        for flick in p.flicks where flick.length > 0.5 {
            var f = Path()
            let c = CGPoint(x: crown.x + flick.at.x, y: crown.y + flick.at.y)
            let d = CGPoint(x: cos(flick.angle) * flick.length, y: sin(flick.angle) * flick.length)
            f.move(to: CGPoint(x: c.x - d.x, y: c.y - d.y))
            f.addQuadCurve(to: CGPoint(x: c.x + d.x, y: c.y + d.y), control: CGPoint(x: c.x - d.y * 0.35, y: c.y + d.x * 0.35))
            pen.stroke(f, width: 2, in: &ctx)
        }

        // The ink drop, falling and stretching; on contact, a ragged splat. Ground is brown.
        if let drop = p.drop {
            if drop.splat {
                var g = ctx
                g.translateBy(x: b.x, y: b.y - 1)
                fillScreen(blob(at: .zero, size: 13, seed: 99, squash: 0.3), reg: .zero, tone: soil, in: &g)
            } else {
                var d = ctx
                d.translateBy(x: b.x, y: b.y + drop.y)
                d.scaleBy(x: drop.sx, y: drop.sy)
                var tear = Path()
                tear.move(to: CGPoint(x: 0, y: -22))
                tear.addQuadCurve(to: CGPoint(x: 8, y: -6), control: CGPoint(x: 6, y: -15))
                tear.addArc(center: CGPoint(x: 0, y: -7), radius: 8, startAngle: .degrees(5), endAngle: .degrees(175), clockwise: false)
                tear.addQuadCurve(to: CGPoint(x: 0, y: -22), control: CGPoint(x: -6, y: -15))
                d.fill(tear, with: .color(paper))
                fillScreen(tear, reg: CGSize(width: 2, height: 2), tone: soil, in: &d)
                pen.stroke(tear, width: 2, in: &d)
            }
        }

        // The fall flicked onto the text box: a dotted pen trail behind it, then the stamp.
        if let fly = p.flying {
            if fly.trail.count > 1 {
                var trail = Path()
                trail.addLines(fly.trail.map { CGPoint(x: b.x + $0.x, y: b.y + $0.y) })
                ctx.stroke(trail, with: .color(ink), style: StrokeStyle(lineWidth: 1.5, lineCap: .round, dash: [2.5, 4]))
            }
            if fly.scale > 0.01 {
                var f = ctx
                f.translateBy(x: b.x + fly.at.x, y: b.y + fly.at.y)
                f.rotate(by: .degrees(fly.spin))
                f.scaleBy(x: fly.scale, y: fly.scale)
                drawPetal(fall(26, 10, curl: 0), beard: true, tone: Tone(tip: 0.6, base: 0.95, ramp: deep), reg: p.register, pen: pen, in: &f)
            }
        }
        if p.stampScale > 0.01 {
            var st = ctx
            st.translateBy(x: b.x + stampAt.x, y: b.y + stampAt.y)
            st.rotate(by: .degrees(-12))
            st.scaleBy(x: p.stampScale, y: p.stampScale)
            if p.ring > 0, p.ring < 1 {
                // The burst as it lands: a pink ring thrown out, thinning, with ink spokes.
                let r = 16 + 5 * CGFloat(p.ring)
                let thin = CGFloat(1 - p.ring)
                let ring = Path(ellipseIn: CGRect(x: -r, y: -r * 0.8, width: 2 * r, height: 1.6 * r))
                fillPink(ring.strokedPath(StrokeStyle(lineWidth: 3.4 * thin)), reg: .zero, in: &st)
                for k in 0..<7 {
                    let a = Double(k) / 7 * 2 * .pi + 0.3
                    var spoke = Path()
                    spoke.move(to: CGPoint(x: cos(a) * (r + 2), y: sin(a) * (r + 2) * 0.8))
                    spoke.addLine(to: CGPoint(x: cos(a) * (r + 2 + 4 * thin), y: sin(a) * (r + 2 + 4 * thin) * 0.8))
                    pen.stroke(spoke, width: 1.8, in: &st)
                }
            }
            st.scaleBy(x: 1.15, y: 1.15)
            let seal = Path(ellipseIn: CGRect(x: -13, y: -9.5, width: 26, height: 19))
            var blot = seal
            blot.addEllipse(in: CGRect(x: 12, y: 5, width: 4.4, height: 3.6))
            st.fill(seal, with: .color(paper))
            fillPink(blot, reg: p.stampRegister, in: &st)
            pen.stroke(seal, width: 1.8, in: &st)
            var tick = Path()
            tick.move(to: CGPoint(x: -7, y: 0))
            tick.addLine(to: CGPoint(x: -1.5, y: 5))
            tick.addQuadCurve(to: CGPoint(x: 9.5, y: -7), control: CGPoint(x: 2.5, y: -1.5))
            pen.stroke(tick, width: 3, in: &st)
        }
    }

    @MainActor private static func drawHead(_ p: IrisPose, pen: Pen, in ctx: inout GraphicsContext) {
        // The page's misregistration, undone for this head's turn and stretch, then a part's.
        func plate(_ angle: Double, _ sx: CGFloat, _ sy: CGFloat, slim: CGFloat = 1) -> CGSize {
            let turn = (p.lean + p.headTurn + angle) * .pi / 180
            let sx = max(0.15, p.squashX * p.headX * p.bud * sx)
            let sy = max(0.15, p.squashY * p.headY * p.bud * sy)
            return CGSize(width: slim * (p.register.width * cos(turn) + p.register.height * sin(turn)) / sx,
                          height: slim * (-p.register.width * sin(turn) + p.register.height * cos(turn)) / sy)
        }
        // Extra petals first, behind everything, then the heart.
        for e in Self.extras {
            let lead = p.petals[e.follow]
            guard lead.length > 0.02 else { continue }
            let o = min(1, max(0, Double((lead.length - 0.15) / 0.85)))
            let angle = e.start + (e.open - e.start) * o
            var c = ctx
            c.rotate(by: .degrees(angle))
            c.scaleBy(x: lead.width, y: lead.length)
            drawPetal(fall(e.length, e.width, curl: 0), beard: false, tone: e.tone, reg: plate(angle, lead.width, lead.length), pen: pen, in: &c)
        }
        // A dotted heart behind the petals, filling the wedges where they meet.
        if p.petals[3].length > 0.02 {
            let r = 10 * p.petals[3].length
            let disc = Path(ellipseIn: CGRect(x: -r, y: -r * 0.95, width: 2 * r, height: 1.9 * r))
            ctx.fill(disc, with: .color(paper))
            fillScreen(disc, reg: plate(0, 1, 1), tone: Tone(tip: 0.85, base: 1, ramp: heartInks), in: &ctx)
        }
        // The outer standards behind, the inner pair, the middle one in front of them, the side
        // falls, then the front fall.
        for i in [1, 2, 6, 7, 0, 3, 4, 5] {
            let petal = p.petals[i]
            guard petal.length > 0.02 else { continue }
            var c = ctx
            c.rotate(by: .degrees(petal.angle))
            c.scaleBy(x: petal.width, y: petal.length)
            let shape: Path
            // Falls deep and dense, standards airy, the outer ones palest: shading across the head.
            let tone: Tone
            switch i {
            case 0, 6, 7: tone = Tone(tip: 0.4, base: 0.85, ramp: soft)
            case 1, 2: tone = Tone(tip: 0.34, base: 0.75, ramp: outer)
            case 5: tone = Tone(tip: 0.6, base: 0.95, ramp: deep)
            default: tone = Tone(tip: 0.7, base: 1, ramp: deep)
            }
            switch i {
            case 0: shape = standard(26, 11, curl: 0)
            case 1: shape = standard(26, 11, curl: 4 * petal.length)
            case 2: shape = standard(26, 11, curl: -4 * petal.length)
            case 6: shape = standard(25, 9.5, curl: 2.5 * petal.length)
            case 7: shape = standard(25, 9.5, curl: -2.5 * petal.length)
            case 5: shape = fall(26, 8.5, curl: 0)
            default: shape = fall(38, 8.5, curl: (i == 3 ? 4 : -4) * petal.length)
            }
            drawPetal(shape, beard: i >= 3 && i <= 5, tone: tone, reg: plate(petal.angle, petal.width, petal.length), pen: pen, in: &c)
        }
        // The closed bud: one fat pink spindle with a twist round it, in a pointed sheath.
        // To open it splits down its seam, the halves swinging apart and shrinking away.
        if p.budBody > 0.01 {
            let split = CGFloat(p.budSplit)
            var spindle = Path()
            spindle.move(to: CGPoint(x: 0, y: 2))
            spindle.addCurve(to: CGPoint(x: 0.5, y: -28), control1: CGPoint(x: -12.5, y: -6), control2: CGPoint(x: -7, y: -22))
            spindle.addCurve(to: CGPoint(x: 0, y: 2), control1: CGPoint(x: 8, y: -22), control2: CGPoint(x: 12.5, y: -6))
            for side in [-1.0, 1.0] {
                var half = ctx
                half.rotate(by: .degrees(side * 70 * Double(split)))
                half.scaleBy(x: p.budBody, y: p.budBody)
                if split > 0.001 { half.clip(to: Path(CGRect(x: side < 0 ? -20 : 0, y: -40, width: 20, height: 50))) }
                half.fill(spindle, with: .color(paper))
                fillScreen(spindle, reg: plate(side * 34 * Double(split), p.budBody, p.budBody, slim: 0.5),
                           tone: Tone(tip: 0.75, base: 1, ramp: budInks), in: &half)
                if split < 0.001 {
                    var wrap = Path()
                    wrap.move(to: CGPoint(x: -6, y: -9))
                    wrap.addQuadCurve(to: CGPoint(x: 4.5, y: -22), control: CGPoint(x: 1, y: -11))
                    pen.stroke(wrap, width: 1.5, in: &half)
                }
                pen.stroke(spindle, width: 2.3, in: &half)
                if split > 0.001 {
                    var seam = Path()
                    seam.move(to: CGPoint(x: 0, y: 2))
                    seam.addLine(to: CGPoint(x: 0.4, y: -29))
                    pen.stroke(seam, width: 2, in: &half)
                }
            }
            if split < 0.5 {
                var sheath = ctx
                sheath.scaleBy(x: 1 - split * 2, y: 1 - split * 2)
                for side in [-1.0, 1.0] {
                    var leafy = Path()
                    leafy.move(to: CGPoint(x: side * -1, y: 4))
                    leafy.addQuadCurve(to: CGPoint(x: side * 6, y: -13), control: CGPoint(x: side * 13, y: -1))
                    leafy.addQuadCurve(to: CGPoint(x: side * 1, y: 4), control: CGPoint(x: side * 4, y: -4))
                    sheath.fill(leafy, with: .color(paper))
                    pen.stroke(leafy, width: 1.6, in: &sheath)
                }
            }
        }
        // The throat where the petals meet: a pinch of pink over the crossing lines, no ring.
        if p.budSplit > 0.5 {
            fillPink(Path(ellipseIn: CGRect(x: -3.2, y: -3.6, width: 6.4, height: 6)), reg: .zero, in: &ctx)
        }
    }

    /// A fall: a narrow haft flaring into a blade with a ruffled tip, curling by `curl`.
    static func fall(_ length: CGFloat, _ width: CGFloat, curl: CGFloat) -> Path {
        let l = length, w = width, c = curl
        var p = Path()
        p.move(to: .zero)
        p.addCurve(to: CGPoint(x: -0.85 * w + c * 0.6, y: -0.68 * l), control1: CGPoint(x: -0.15 * w, y: -0.25 * l),
                   control2: CGPoint(x: -0.95 * w + c * 0.3, y: -0.42 * l))
        p.addQuadCurve(to: CGPoint(x: -0.3 * w + c, y: -0.98 * l), control: CGPoint(x: -0.85 * w + c, y: -0.97 * l))
        p.addQuadCurve(to: CGPoint(x: 0.3 * w + c, y: -0.98 * l), control: CGPoint(x: c, y: -1.1 * l))
        p.addQuadCurve(to: CGPoint(x: 0.85 * w + c * 0.6, y: -0.68 * l), control: CGPoint(x: 0.85 * w + c, y: -0.97 * l))
        p.addCurve(to: .zero, control1: CGPoint(x: 0.95 * w + c * 0.3, y: -0.42 * l),
                   control2: CGPoint(x: 0.15 * w, y: -0.25 * l))
        return p
    }

    /// A standard: pinched at the base, a cupped spoon at the top, its tip leaning in by `curl`.
    static func standard(_ length: CGFloat, _ width: CGFloat, curl: CGFloat) -> Path {
        var p = Path()
        p.move(to: .zero)
        p.addCurve(to: CGPoint(x: -width * 0.5 + curl, y: -length * 0.86), control1: CGPoint(x: -width * 0.15, y: -length * 0.3),
                   control2: CGPoint(x: -width * 0.75 + curl * 0.6, y: -length * 0.6))
        p.addQuadCurve(to: CGPoint(x: width * 0.5 + curl, y: -length * 0.86), control: CGPoint(x: curl * 1.3, y: -length * 1.18))
        p.addCurve(to: .zero, control1: CGPoint(x: width * 0.75 + curl * 0.6, y: -length * 0.6),
                   control2: CGPoint(x: width * 0.15, y: -length * 0.3))
        return p
    }

    /// An irregular grained blot, like a fleck of spilled ink.
    static func blob(at c: CGPoint, size r: CGFloat, seed: Int, squash: CGFloat = 0.85) -> Path {
        var p = Path()
        let n = 7
        let pts = (0..<n).map { i -> CGPoint in
            let a = Double(i) / Double(n) * 2 * .pi
            let rr = r * CGFloat(1 + 0.28 * Curve.noise(seed * 31 + i))
            return CGPoint(x: c.x + cos(a) * rr, y: c.y + sin(a) * rr * squash)
        }
        p.move(to: CGPoint(x: (pts[0].x + pts[n - 1].x) / 2, y: (pts[0].y + pts[n - 1].y) / 2))
        for i in 0..<n {
            let next = pts[(i + 1) % n]
            p.addQuadCurve(to: CGPoint(x: (pts[i].x + next.x) / 2, y: (pts[i].y + next.y) / 2), control: pts[i])
        }
        p.closeSubpath()
        return p
    }

    /// Paper under the line (so parts in front hide the ones behind), pink off register, then ink.
    @MainActor private static func drawPetal(_ shape: Path, beard: Bool, tone: Tone, reg: CGSize, pen: Pen,
                                             in ctx: inout GraphicsContext) {
        ctx.fill(shape, with: .color(paper))
        fillScreen(shape, reg: reg, tone: tone, in: &ctx)
        if beard {
            // The fall's pale throat and its beard: an ink line with short hairs down the haft.
            let box = shape.boundingRect
            ctx.fill(Path(ellipseIn: CGRect(x: -box.width * 0.13, y: box.minY * 0.5, width: box.width * 0.26, height: -box.minY * 0.36)),
                     with: .color(paper))
            for (k, f) in ([0.16, 0.22, 0.28, 0.34, 0.4, 0.46] as [CGFloat]).enumerated() {
                let x: CGFloat = k % 2 == 0 ? -0.8 : 0.8
                ctx.fill(Path(ellipseIn: CGRect(x: x - 1.1, y: box.minY * f - 1.1, width: 2.2, height: 2.2)), with: .color(ink))
            }
        }
        pen.stroke(shape, width: 2.4, in: &ctx)
    }

    /// Dots landed `reg` off the line, in five bands from the tip to the base, each a step deeper
    /// and a step along the ink ramp, printed over the last so the inks mix where they meet. The
    /// part drawn grows from the origin, so shrinking it toward the origin finds its base.
    @MainActor static func fillScreen(_ path: Path, reg: CGSize, tone: Tone, in ctx: inout GraphicsContext) {
        let moved = path.offsetBy(dx: reg.width, dy: reg.height)
        var g = ctx
        g.clip(to: moved)
        let bands: [(shrink: CGFloat, m: CGFloat)] = [(1, 0), (0.84, 0.25), (0.69, 0.5), (0.55, 0.75), (0.42, 1)]
        for band in bands {
            let area = band.shrink == 1 ? Path(moved.boundingRect.insetBy(dx: -4, dy: -4))
                : path.applying(CGAffineTransform(scaleX: band.shrink, y: band.shrink)).offsetBy(dx: reg.width, dy: reg.height)
            let ink = along(tone.ramp, band.m)
            g.fill(area, with: .tiledImage(dots(ink, tone.tip + (tone.base - tone.tip) * band.m), scale: dotScale))
        }
    }

    /// The ink `m` (0–1) of the way along a ramp.
    static func along(_ ramp: [RGB], _ m: CGFloat) -> RGB {
        guard ramp.count > 1 else { return ramp[0] }
        let x = min(1, max(0, m)) * CGFloat(ramp.count - 1)
        let i = min(ramp.count - 2, Int(x)), f = x - CGFloat(i)
        let a = ramp[i], b = ramp[i + 1]
        return RGB((a.r + (b.r - a.r) * f).rounded(), (a.g + (b.g - a.g) * f).rounded(), (a.b + (b.b - a.b) * f).rounded())
    }

    /// Flat pink landed `reg` off the line, with the print's coarse grain through it.
    @MainActor static func fillPink(_ path: Path, reg: CGSize, in ctx: inout GraphicsContext) {
        let moved = path.offsetBy(dx: reg.width, dy: reg.height)
        ctx.fill(moved, with: .color(pink))
        var g = ctx
        g.clip(to: moved)
        let area = Path(moved.boundingRect.insetBy(dx: -4, dy: -4))
        g.opacity = 0.18
        g.fill(area, with: .tiledImage(salt, scale: 0.5))
        g.opacity = 0.45
        g.fill(area, with: .tiledImage(grain, scale: 0.9))
    }

    /// The pen: a heavy round ink line that boils (shifts a hair, a few times a second), gone
    /// over once more a hair off so it reads as pressed.
    struct Pen {
        let seed: Int

        private func jitter(_ path: Path) -> CGSize {
            let r = path.boundingRect
            let key = seed &* 7919 &+ Int(r.minX * 3) &* 131 &+ Int(r.minY * 3) &* 17 &+ Int(r.width * 3)
            return CGSize(width: 0.35 * Curve.noise(key), height: 0.35 * Curve.noise(key &+ 1))
        }

        func stroke(_ path: Path, width: CGFloat, in ctx: inout GraphicsContext) {
            let j = jitter(path)
            let line = path.offsetBy(dx: j.width, dy: j.height)
            ctx.stroke(line, with: .color(IrisRig.ink), style: StrokeStyle(lineWidth: width, lineCap: .round, lineJoin: .round))
            ctx.stroke(line.offsetBy(dx: 0.4, dy: 0.35), with: .color(IrisRig.ink.opacity(0.35)),
                       style: StrokeStyle(lineWidth: width * 0.7, lineCap: .round, lineJoin: .round))
        }

        /// A cubic drawn in short pieces whose width runs from `widths.0` to `widths.1`.
        func taper(from p0: CGPoint, _ p1: CGPoint, _ p2: CGPoint, _ p3: CGPoint, widths: (CGFloat, CGFloat),
                   color: Color = IrisRig.ink, in ctx: inout GraphicsContext) {
            func at(_ t: CGFloat) -> CGPoint {
                let u = 1 - t
                return CGPoint(x: u * u * u * p0.x + 3 * u * u * t * p1.x + 3 * u * t * t * p2.x + t * t * t * p3.x,
                               y: u * u * u * p0.y + 3 * u * u * t * p1.y + 3 * u * t * t * p2.y + t * t * t * p3.y)
            }
            let n = 14
            let j = 0.3 * Curve.noise(seed &* 3)
            for i in 0..<n {
                var s = Path()
                s.move(to: at(CGFloat(i) / CGFloat(n)))
                s.addLine(to: at(CGFloat(i + 1) / CGFloat(n)))
                let w = widths.0 + (widths.1 - widths.0) * CGFloat(i) / CGFloat(n - 1)
                ctx.stroke(s.offsetBy(dx: j, dy: 0), with: .color(color), style: StrokeStyle(lineWidth: w, lineCap: .round))
            }
        }
    }
}

// MARK: - The pose

/// Every part's state at one moment, worked out from the clock.
struct IrisPose {
    struct PetalPose { var angle = 0.0; var width: CGFloat = 0; var length: CGFloat = 0 }
    struct Speck { var at: CGPoint; var size: CGFloat }
    struct Flick { var at: CGPoint; var angle: CGFloat; var length: CGFloat }
    struct Drop { var y: CGFloat; var sx: CGFloat; var sy: CGFloat; var splat = false }
    struct Flying { var at: CGPoint; var spin: Double; var scale: CGFloat; var trail: [CGPoint] }

    var puddle: CGFloat = 1
    var flecks: [Speck] = []
    var growth: CGFloat = 1
    var stretch: CGFloat = 1
    var bend: CGFloat = 0
    var curl: CGFloat = 0
    var leaf: CGFloat = 1
    var leafTurn = 0.0
    var bud: CGFloat = 1
    var headShown = true
    var headX: CGFloat = 1, headY: CGFloat = 1
    var headTurn = 0.0
    var lean = 0.0
    var sink: CGFloat = 0
    var squashX: CGFloat = 1, squashY: CGFloat = 1
    var register = CGSize(width: 3, height: 2.6)
    /// The closed bud's spindle (its size), and how far it has split open.
    var budBody: CGFloat = 1
    var budSplit = 0.0
    var petals = Array(repeating: PetalPose(), count: 8)
    var dots: [Speck] = []
    var flicks: [Flick] = []
    var drop: Drop?
    var flying: Flying?
    var stampScale: CGFloat = 0
    /// The landing burst, 0 → 1 (drawn only in between).
    var ring = 0.0
    var stampRegister = CGSize(width: 1.4, height: 1.4)
    /// Which pen wobble is showing: changes eight times a second, and stops once you're reading.
    var boil = 0

    /// Open angles: five standards fanned from the middle out to nearly level (indices 0, 7/6
    /// at 35°, 1/2 at 70°), filling the space above the falls, which hang about 30° below
    /// level; the front fall straight down.
    static let openAngle: [Double] = [0, -70, 70, -122, 122, 180, -35, 35]
    /// Where each part starts as it grows out of the bud: falls already pointing down and out,
    /// so they only ever travel downward.
    static let startAngle: [Double] = [0, -30, 30, -100, 100, 180, -10, 10]

    private static let crown: [(vx: CGFloat, vy: CGFloat, size: CGFloat, stays: Bool)] = [
        (-135, 150, 5.0, false), (-225, 110, 3.6, true), (-75, 190, 3.0, false), (-285, 80, 2.8, true),
        (120, 160, 4.2, false), (210, 115, 3.2, true), (60, 200, 2.6, false), (280, 70, 2.4, true),
    ]

    init(_ c: RigClock) {
        let a = c.since(c.arrived) ?? 99
        let b = c.since(c.bloomed)
        let w = c.since(c.wilted)
        let full = c.full
        let t = c.now
        boil = Int(floor(t * 8))

        // MARK: Arrival
        // Full: the drop hangs and wobbles (60 ms), falls stretching, lands at 200 ms as a
        // ragged splat, throws its crown, the stem shoots up out of the puddle with the bud on
        // its tip, the sword leaf unfurls, the pink lands 6 pt off and snaps to rest at 600 ms.
        // Quick: the stem and bud are up by 80 ms and the pink snaps by 160 ms.
        let land = full ? 0.2 : 0.0
        if full && a < land + 0.08 {
            if a < land {
                let x = Curve.clamp((a - 0.06) / (land - 0.06))
                let f = x * x
                let wobble = a < 0.06 ? 0.08 * sin(a / 0.06 * .pi * 2) : 0
                drop = Drop(y: CGFloat(-112 * (1 - f)), sx: CGFloat(1.25 * (1 - wobble) * (1 - 0.3 * f)),
                            sy: CGFloat(1.25 * (1 + wobble) * (1 + 0.6 * f)))
            } else {
                drop = Drop(y: 0, sx: 1, sy: 1, splat: true)
            }
        }
        if full {
            let after = a - land
            flecks = Self.crown.map { c in
                guard after > 0 else { return Speck(at: .zero, size: 0) }
                let s = CGFloat(after)
                let y = -c.vy * s + 900 * s * s
                if y >= 0 && s > 0.05 {
                    // Landed: the big ones vanish into the puddle, a few stay as specks.
                    let landT = c.vy / 900
                    return Speck(at: CGPoint(x: c.vx * landT, y: 1), size: c.stays ? c.size * 0.75 : 0)
                }
                return Speck(at: CGPoint(x: c.vx * s, y: y), size: c.size)
            }
        }
        puddle = full ? CGFloat(Curve.spring(a - land - 0.04, 0.24, bounce: 0.3)) : CGFloat(Curve.easeOut(a / 0.1)) * 0.65
        let growAt = full ? land + 0.07 : 0.0
        let grow = Curve.spring(a - growAt, full ? 0.26 : 0.15, bounce: 0.4)
        growth = CGFloat(min(1, grow))
        stretch = CGFloat(1 + max(0, grow - 1) * 1.1)
        bend = CGFloat(1 - Curve.easeOut((a - growAt - 0.05) / 0.3))
        bud = CGFloat(Curve.spring(a - growAt, full ? 0.24 : 0.1, bounce: 0.3))
        headShown = bud > 0.02
        let leafAt = full ? growAt + 0.12 : 0.04
        let u = Curve.spring(a - leafAt, full ? 0.34 : 0.14, bounce: 0.5)
        leaf = CGFloat(u)
        leafTurn = (1 - u) * 40
        let snapAt = full ? 0.6 : 0.1
        let r = a < snapAt ? 1 : 1 - Curve.spring(a - snapAt, 0.12, bounce: 0.5)
        register = CGSize(width: 3 + 3.6 * r, height: 2.6 + 3 * r)

        // MARK: Reading
        // Leans in toward your text (14°), the stem bending with a ±5° sway; the head cocks 10°
        // further in, trails the stem, and nods (11°, with a little squash first) every 0.8 s,
        // the pink jolting off register on each nod. After 1.2 s it works visibly harder.
        func sway(_ t: Double) -> Double { sin(t * 2 * .pi / 1.6) * 5 }
        let settled = full ? 0.6 : 0.16
        let busy = 1 + 0.4 * Curve.easeInOut((a - settled - 1.2) / 0.6)
        func readLean(_ t: Double) -> Double { 5 + sway(t) * 0.3 * busy }
        func readCurl(_ t: Double) -> CGFloat { CGFloat((12 + sway(t) * busy) * 2.4) }
        func readTurn(_ t: Double) -> Double {
            let beat = t.truncatingRemainder(dividingBy: 0.8)
            let nod = 11 * busy * Curve.bump(beat - 0.1, 0.24)
            return 16 + (sway(t - 0.1) - sway(t)) * 1.5 + nod
        }
        let reading = full ? Curve.easeOut((a - settled) / 0.2) : Curve.easeInOut((a - settled - 0.25) / 0.4)
        if b == nil && w == nil {
            lean = readLean(t) * reading
            curl = readCurl(t) * CGFloat(reading)
            headTurn = readTurn(t) * reading
            let beat = t.truncatingRemainder(dividingBy: 0.8)
            squashY = CGFloat(1 - 0.05 * Curve.bump(beat, 0.12) * reading)
            let jolt = Curve.bump(beat - 0.1, 0.24) * reading
            register.width += CGFloat(1.6 * jolt); register.height += CGFloat(1.2 * jolt)
            let breath = 1 + sin(t * 2 * .pi / 0.8) * 0.04 * busy * reading
            headX = CGFloat(1 / breath); headY = CGFloat(breath)
        }

        // MARK: Bloom
        // Full: from its reading pose the bud leans back and squashes (120 ms), splits down its
        // seam, the falls grow out of it already pointing down and swing further down and out
        // (left, right, front, 50 ms apart) while the standards rise with them; the head swells
        // a little and settles, the pop flies out left. When the panel lands beside it the
        // flower recoils, idles for a moment, then keeps still while you read.
        // Quick: no lean back; it splits and opens in about 140 ms, before the panel lands.
        let gather = full ? 0.14 : 0.0
        var open = [Double](repeating: 0, count: 8)
        if let b {
            let stagger: [Double] = full ? [0.16, 0.08, 0.11, 0.02, 0.09, 0.16, 0.12, 0.14]
                : [0.03, 0.02, 0.025, 0, 0.01, 0.02, 0.025, 0.03]
            for i in 0..<8 {
                open[i] = i < 3 || i > 5 ? Curve.spring(b - gather - stagger[i], full ? 0.3 : 0.14, bounce: full ? 0.45 : 0.3)
                    : Curve.spring(b - gather - stagger[i], full ? 0.32 : 0.14, bounce: full ? 0.45 : 0.3)
            }
            budSplit = Curve.easeOut((b - gather) / (full ? 0.08 : 0.05))
            budBody = CGFloat(1 - Curve.easeOut((b - gather) / (full ? 0.08 : 0.05)))
            let after = PanelMotion.afterBloom(full: full)
            let squash = full ? Curve.bump(b, gather * 1.6) : 0
            headY = CGFloat(1 - 0.16 * squash + (full ? 0.15 : 0) * Curve.bump(b - gather, 0.12)); headX = CGFloat(1 + 0.12 * squash)
            let swell = 1 + (full ? 0.2 : 0.1) * Curve.bump(b - gather, full ? 0.34 : 0.22)
                - (full ? 0.04 : 0.02) * Curve.bump(b - gather - (full ? 0.3 : 0.2), 0.24)
            headX *= CGFloat(swell); headY *= CGFloat(swell)
            let recoil = Curve.bump(b - after - 0.08, 0.26)
            // From wherever reading left it, back (anticipation), then up and settled.
            let readAtBloom = Curve.easeOut((a - b - settled) / 0.2)
            let from = c.bloomed.map { readLean($0) * readAtBloom } ?? 0
            let handOff = Curve.easeInOut(b / (full ? 0.2 : 0.1))
            // A little idling once open, then still while you read the panel.
            let idle = Curve.easeOut((b - 0.5) / 0.4) * (1 - Curve.easeInOut((b - 1.6) / 0.5))
            lean = from * (1 - handOff) + (4 - (full ? 12 : 0) * Curve.bump(b, gather * 1.8)) * handOff - 9 * recoil
                + 2.5 * sin(t * 2 * .pi / 2.4) * idle
            curl = CGFloat(Double(c.bloomed.map { readCurl($0) } ?? 0) * readAtBloom * (1 - handOff) - 6 * recoil)
            headTurn = (c.bloomed.map { readTurn($0) } ?? 0) * readAtBloom * (1 - handOff) - 10 * recoil
            headY *= CGFloat(1 - 0.06 * recoil)
            for i in 3..<6 { open[i] += 0.03 * sin(t * 2 * .pi / 1.8 + Double(i)) * idle }
            if b > 2.1, let bloomed = c.bloomed { boil = Int(floor((bloomed + 2.1) * 8)) }
            stretch *= CGFloat(1 + 0.08 * Curve.bump(b - gather, 0.3))
            leafTurn -= 12 * Curve.bump(b - gather - 0.06, 0.34)
            // The pop: mixed sizes, out to the left and up, away from the panel; they dry up.
            let count = full ? 9 : 2
            let popAt = gather + (full ? 0.1 : 0.05)
            let p = Curve.easeOut((b - popAt) / 0.2)
            let dry = 1 - Curve.easeIn((b - popAt - (full ? 0.35 : 0.12)) / (full ? 0.2 : 0.12))
            if b > popAt && dry > 0 {
                let sizes: [CGFloat] = [2.6, 4.2, 2.0, 3.4, 4.8, 2.4, 3.8, 2.2, 3.0]
                dots = (0..<count).map { i in
                    let angle = Double.pi * (0.62 + 0.8 * Double(i) / Double(max(1, count - 1)))
                    let reach = 30 * (1 + 0.6 * Curve.noise(i * 13 + 5) * 0.5 + 0.3) * p
                    return Speck(at: CGPoint(x: cos(angle) * reach, y: sin(angle) * reach), size: sizes[i] * CGFloat(dry))
                }
                if full {
                    flicks = (0..<3).map { i in
                        let angle = Double.pi * [1.05, 1.32, 0.8][i]
                        let reach = 42 * p
                        return Flick(at: CGPoint(x: cos(angle) * reach, y: sin(angle) * reach), angle: CGFloat(angle), length: 5 * CGFloat(dry))
                    }
                }
            }
        }

        // MARK: Done
        // Full: a quick nod, the front fall tips up and flies on one high arc with a full turn,
        // a dotted pen trail behind it (rubbed out after), and lands above the box's top edge
        // as a stamp with a tick: it overshoots, throws a pink ring, snaps into register. Then
        // the plant stretches, folds back into its bud and sinks into its puddle, the puddle
        // dries, and the stamp holds 0.6 s and pops away. None of it over your text.
        // Quick: no flight; the stamp lands at once and the plant goes with it, about 0.5 s.
        // Dismissed without replacing: no flight and no stamp, the plant just folds and sinks.
        var landed = -1.0
        var shutAt = 0.04
        if let w {
            if full && c.replaced {
                lean = 4 - 6 * Curve.bump(w, 0.07) + 8 * Curve.bump(w - 0.06, 0.3)
                curl = CGFloat(4 * Curve.bump(w - 0.06, 0.3))
                headTurn = -12 * Curve.bump(w - 0.05, 0.1) + 10 * Curve.bump(w - 0.12, 0.3)
                open[5] += 0.12 * Curve.bump(w, 0.08)
                let flyFrom = 0.06, flyFor = 0.34
                let from = CGPoint(x: 0, y: -IrisRig.stem + 16)
                let to = IrisRig.stampAt
                func arc(_ f: CGFloat) -> CGPoint {
                    // One parabola: up well above the head, then down onto the box's edge.
                    CGPoint(x: from.x + (to.x - from.x) * f, y: from.y + (to.y - from.y) * f * f - 360 * f * (1 - f))
                }
                let f = CGFloat(Curve.clamp((w - flyFrom) / flyFor))
                let inFlight = w > flyFrom && w < flyFrom + flyFor
                if w > flyFrom && w < flyFrom + flyFor + 0.14 {
                    let tail = inFlight ? max(0, f - 0.5) : CGFloat(Curve.easeIn((w - flyFrom - flyFor) / 0.14))
                    flying = Flying(at: arc(f), spin: Double(f) * 360, scale: inFlight ? 1 - 0.15 * f : 0,
                                    trail: stride(from: tail, through: f, by: 0.035).map { arc($0) })
                }
                if w > flyFrom { open[5] = 0 }
                landed = w - flyFrom - flyFor
                shutAt = flyFrom + flyFor
            } else {
                lean = 6
                if c.replaced {
                    if w > 0.04 { open[5] = 0 }
                    landed = w - 0.04
                }
            }
            if landed > 0 {
                let hold = full ? 0.6 : 0.28
                stampScale = CGFloat((0.55 + 0.45 * Curve.spring(landed, 0.18, bounce: 0.7)) * (1 - Curve.easeIn((landed - hold) / 0.14)))
                ring = Curve.easeOut(landed / 0.22)
                let r = 1 - Curve.spring(landed - 0.05, 0.12, bounce: 0.5)
                stampRegister = CGSize(width: 1.4 + 3 * r, height: 1.4 + 2.5 * r)
            }
            // The plant stretches up, folds its petals back into the bud and sinks into its ink.
            let up = w - shutAt
            let shut = Curve.easeInOut(up / (full ? 0.18 : 0.12))
            for i in 0..<8 { open[i] *= 1 - shut }
            budSplit = 1 - shut
            budBody = CGFloat(shut)
            squashY = CGFloat(1 + 0.07 * Curve.bump(up, 0.14))
            sink = IrisRig.stem * 1.7 * CGFloat(Curve.easeIn((up - (full ? 0.1 : 0.06)) / (full ? 0.26 : 0.18)))
            if sink > IrisRig.stem * 1.65 { headShown = false }
            let dry = 1 - Curve.easeIn((up - (full ? 0.3 : 0.2)) / 0.2)
            puddle *= CGFloat(dry)
            flecks = flecks.map { Speck(at: $0.at, size: $0.size * CGFloat(dry)) }
        }

        // Each part grows out of the bud along its own path.
        for i in 0..<8 {
            let o = open[i]
            guard o > 0.001 else { petals[i] = PetalPose(); continue }
            petals[i].angle = Self.startAngle[i] + (Self.openAngle[i] - Self.startAngle[i]) * o
            petals[i].length = CGFloat(0.15 + 0.85 * min(1.06, o))
            petals[i].width = CGFloat(0.3 + 0.7 * min(1.04, o))
        }
    }
}
