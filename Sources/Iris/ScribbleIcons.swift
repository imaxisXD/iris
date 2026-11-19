import SwiftUI

/* ─────────────────────────────────────────────────────────
 * SCRIBBLE CRITTERS · the icons on About's highlighted phrases
 *
 * Each catch drawn as a little crayon creature: its shape scribbled
 * in the sky ink with dense looping strokes (paper showing through
 * the gaps), a rough crayon edge, big glossy eyes, and one prop in
 * sunflower yellow. Static strokes (seeded), so nothing jitters.
 *
 *   idle      blinks on its own beat for five seconds after it appears or
 *             arrives, then rests with its eyes open (WCAG 2.2.2)
 *   mood      each is a catch, so none of them is happy: the price tag
 *             and the lock are grumpy, the cloud and the chip side-eye
 *             you, the Wi-Fi and the plane are sad, the clock and the
 *             archive are worn out
 *   arrives   winces (> <) for a moment while a dark grumble scribble
 *             puffs above it (and a sweat drop, if it's sad), then fades
 *
 * Reduce Motion: its mood, still; no blinking, no wince, no grumble.
 * ───────────────────────────────────────────────────────── */

enum Scribble {
    static let blue = Color(red: 120 / 255, green: 162 / 255, blue: 220 / 255)
    static let deepBlue = Color(red: 78 / 255, green: 118 / 255, blue: 190 / 255)
    static let yellow = Color(red: 250 / 255, green: 196 / 255, blue: 60 / 255)
    static let ink = Color(red: 22 / 255, green: 16 / 255, blue: 14 / 255)

    static let blinkEvery = 2.4     // seconds between blinks
    static let livelyFor = 5.0      // it blinks only this long after appearing or arriving
    static let blinkTakes = 0.14
    static let winceFor = 0.4       // the > < after arriving
    static let grumbleFor = 1.2     // the dark scribble puff after arriving
}

/// A catch's icon as a scribble creature. `since` is when it came on (after a click).
struct ScribbleCritter: View {
    let icon: RisoIcon
    var size: CGFloat = 34
    var since: Date = .distantPast
    @State private var lively = true
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 24, paused: reduceMotion || !lively || OnDisplay.isOn)) { ctx in
            let now = OnDisplay.isOn ? 1000 : ctx.date.timeIntervalSinceReferenceDate
            Canvas { g, canvas in
                let k = size / 64
                g.translateBy(x: (canvas.width - size) / 2, y: (canvas.height - size) / 2)
                g.scaleBy(x: k, y: k)
                draw(&g, now: now)
            }
        }
        // Room round it for the sparkles.
        .frame(width: size * 1.7, height: size * 1.7)
        .frame(width: size, height: size)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
        .task(id: since) {
            lively = true
            try? await Task.sleep(for: .seconds(Scribble.livelyFor))
            lively = false
        }
    }

    private var seed: Int { icon.rawValue.unicodeScalars.reduce(5) { $0 &* 31 &+ Int($1.value) } & 0xffff }

    private func draw(_ g: inout GraphicsContext, now: Double) {
        // Once it has settled its timeline is stopped and its date is stale: draw it at rest.
        let arrived = lively ? now - since.timeIntervalSinceReferenceDate : 99
        let shape = Self.body(icon)
        // Props behind.
        drawProp(&g, behind: true)
        // The body, scribbled.
        g.drawLayer { l in
            l.clip(to: shape)
            l.stroke(Self.scribble(in: shape.boundingRect, seed: seed, loops: 26), with: .color(Scribble.blue),
                     style: StrokeStyle(lineWidth: 3.4, lineCap: .round, lineJoin: .round))
            l.stroke(Self.scribble(in: shape.boundingRect, seed: seed + 7, loops: 10), with: .color(Scribble.deepBlue.opacity(0.55)),
                     style: StrokeStyle(lineWidth: 2.2, lineCap: .round, lineJoin: .round))
        }
        g.stroke(Self.rough(shape, seed: seed), with: .color(Scribble.blue), style: StrokeStyle(lineWidth: 3.6, lineCap: .round, lineJoin: .round))
        drawProp(&g, behind: false)
        drawEyes(&g, now: now, arrived: arrived)
        drawGrumble(&g, arrived: arrived)
    }

    // MARK: Face

    enum Mood { case grumpy, sideEye, sad, tired }

    static func mood(_ icon: RisoIcon) -> Mood {
        switch icon {
        case .priceTag, .privacy: .grumpy
        case .watchingCloud, .model: .sideEye
        case .wifi, .paperPlane: .sad
        default: .tired
        }
    }

    private func drawEyes(_ g: inout GraphicsContext, now: Double, arrived: Double) {
        let c0 = Self.eyes(icon)
        // Big eyes, like the reference's: most of the face.
        let c = (at: c0.at, gap: c0.gap * 1.45, r: c0.r * 1.5)
        let mood = Self.mood(icon)
        let wince = !reduceMotion && arrived < Scribble.winceFor
        let phase = (now + Double(seed % 97) * 0.13).truncatingRemainder(dividingBy: Scribble.blinkEvery)
        let blink = reduceMotion || !lively ? 1 : (phase < Scribble.blinkTakes ? max(0.12, abs(phase / Scribble.blinkTakes * 2 - 1)) : 1)
        // How far the upper lid comes down: none, or half for the side-eye and the worn out.
        let lid: CGFloat = mood == .sideEye ? 0.45 : (mood == .tired ? 0.55 : 0)
        let look: CGFloat = mood == .sideEye ? (icon == .model ? -2.6 : 2.6) : 0
        let line = StrokeStyle(lineWidth: 3.2, lineCap: .round, lineJoin: .round)
        for (side, dx) in [(-1.0, -c.gap / 2), (1.0, c.gap / 2)] as [(CGFloat, CGFloat)] {
            let p = CGPoint(x: c.at.x + dx, y: c.at.y)
            if wince {
                // > <: each eye squeezed shut toward the middle.
                var v = Path()
                v.move(to: CGPoint(x: p.x - side * c.r, y: p.y - c.r * 0.8))
                v.addLine(to: CGPoint(x: p.x + side * c.r * 0.6, y: p.y))
                v.addLine(to: CGPoint(x: p.x - side * c.r, y: p.y + c.r * 0.8))
                g.stroke(v, with: .color(Scribble.ink), style: line)
                continue
            }
            let h = c.r * 1.15 * CGFloat(blink)
            let eye = Path(ellipseIn: CGRect(x: p.x - c.r, y: p.y - h, width: c.r * 2, height: h * 2))
            g.fill(eye, with: .color(Scribble.ink))
            if blink > 0.6 {
                let w = c.r * 0.5
                g.fill(Path(ellipseIn: CGRect(x: p.x - c.r * 0.45 + look, y: p.y - c.r * 0.55 + (lid > 0 ? h * lid : 0), width: w, height: w)), with: .color(.white))
            }
            // The upper lid, in the body's blue, with its ink edge.
            if lid > 0 {
                let top = p.y - h, edge = p.y - h + h * 2 * lid
                g.drawLayer { l in
                    l.clip(to: eye)
                    l.fill(Path(CGRect(x: p.x - c.r - 1, y: top - 1, width: c.r * 2 + 2, height: edge - top + 1)), with: .color(Scribble.blue))
                }
                g.stroke(Path { $0.move(to: CGPoint(x: p.x - c.r, y: edge)); $0.addLine(to: CGPoint(x: p.x + c.r, y: edge)) },
                         with: .color(Scribble.ink), style: StrokeStyle(lineWidth: 2.4, lineCap: .round))
            }
            // Brows: grumpy ones drop toward the middle, sad ones rise toward it.
            if mood == .grumpy || mood == .sad {
                let inner = CGPoint(x: p.x - side * c.r * 0.9, y: p.y - h - (mood == .grumpy ? 2 : 8))
                let outer = CGPoint(x: p.x + side * c.r * 0.9, y: p.y - h - (mood == .grumpy ? 8 : 3))
                g.stroke(Path { $0.move(to: inner); $0.addLine(to: outer) }, with: .color(Scribble.ink), style: line)
            }
        }
        // The mouth: a frown, wobbly when sad, flat when side-eyeing.
        let m = CGPoint(x: c.at.x, y: c.at.y + c.r * 1.55 + 3)
        var mouth = Path()
        switch mood {
        case .sideEye:
            mouth.move(to: CGPoint(x: m.x - 4 + look, y: m.y)); mouth.addLine(to: CGPoint(x: m.x + 4 + look, y: m.y - 0.5))
        case .sad:
            mouth.move(to: CGPoint(x: m.x - 6, y: m.y + 2))
            mouth.addQuadCurve(to: CGPoint(x: m.x, y: m.y - 1), control: CGPoint(x: m.x - 3, y: m.y - 2))
            mouth.addQuadCurve(to: CGPoint(x: m.x + 6, y: m.y + 2), control: CGPoint(x: m.x + 3, y: m.y - 2))
        default:
            mouth.move(to: CGPoint(x: m.x - 5, y: m.y + 2))
            mouth.addQuadCurve(to: CGPoint(x: m.x + 5, y: m.y + 2), control: CGPoint(x: m.x, y: m.y - 3))
        }
        g.stroke(mouth, with: .color(Scribble.ink), style: StrokeStyle(lineWidth: 2.6, lineCap: .round, lineJoin: .round))
    }

    // MARK: Grumble

    /// After arriving: a small dark scribble puff above it, the comic sign of a grumble, and a
    /// sweat drop beside a sad one; both fade.
    private func drawGrumble(_ g: inout GraphicsContext, arrived: Double) {
        guard !reduceMotion, arrived < Scribble.grumbleFor else { return }
        let k = arrived / Scribble.grumbleFor
        let pop = Curve.spring(arrived, 0.3, bounce: 0.3)
        let fade = 1 - Curve.easeIn(k)
        // On its own head, never up into the line of words above it.
        let puff = CGRect(x: 44, y: 2 - 2 * CGFloat(k), width: 17 * CGFloat(pop), height: 11 * CGFloat(pop))
        g.stroke(Self.scribble(in: puff, seed: seed + 41, loops: 8), with: .color(Scribble.ink.opacity(0.7 * fade)),
                 style: StrokeStyle(lineWidth: 1.8, lineCap: .round, lineJoin: .round))
        if Self.mood(icon) == .sad {
            let y = 18 + 14 * CGFloat(k)
            var drop = Path()
            drop.move(to: CGPoint(x: 6, y: y - 7))
            drop.addQuadCurve(to: CGPoint(x: 6, y: y + 4), control: CGPoint(x: 0, y: y + 2))
            drop.addQuadCurve(to: CGPoint(x: 6, y: y - 7), control: CGPoint(x: 12, y: y + 2))
            g.fill(drop, with: .color(Scribble.blue.opacity(fade)))
            g.stroke(drop, with: .color(Scribble.ink.opacity(0.6 * fade)), lineWidth: 1.4)
        }
    }

    // MARK: Props, in yellow crayon

    private func drawProp(_ g: inout GraphicsContext, behind: Bool) {
        let crayon = StrokeStyle(lineWidth: 3.8, lineCap: .round, lineJoin: .round)
        func fill(_ p: Path) {
            g.drawLayer { l in
                l.clip(to: p)
                l.stroke(Self.scribble(in: p.boundingRect, seed: seed + 3, loops: 14), with: .color(Scribble.yellow), style: crayon)
            }
            g.stroke(p, with: .color(Scribble.yellow), style: StrokeStyle(lineWidth: 2.6, lineCap: .round, lineJoin: .round))
        }
        switch (icon, behind) {
        case (.priceTag, false):            // its string, looping off the hole
            var s = Path(); s.move(to: CGPoint(x: 22, y: 33)); s.addCurve(to: CGPoint(x: 4, y: 8), control1: CGPoint(x: 8, y: 34), control2: CGPoint(x: 0, y: 20))
            g.stroke(s, with: .color(Scribble.yellow), style: crayon)
            g.fill(Path(ellipseIn: CGRect(x: 18, y: 29, width: 8, height: 8)), with: .color(DS.Surface.top))
        case (.clock, true):                // two bells
            fill(Path(ellipseIn: CGRect(x: 6, y: 6, width: 18, height: 16)))
            fill(Path(ellipseIn: CGRect(x: 40, y: 6, width: 18, height: 16)))
        case (.clock, false):               // little feet
            g.stroke(Path { $0.move(to: CGPoint(x: 18, y: 54)); $0.addLine(to: CGPoint(x: 13, y: 61)); $0.move(to: CGPoint(x: 46, y: 54)); $0.addLine(to: CGPoint(x: 51, y: 61)) },
                     with: .color(Scribble.yellow), style: crayon)
        case (.privacy, true):              // the shackle
            var a = Path(); a.addArc(center: CGPoint(x: 32, y: 28), radius: 13, startAngle: .degrees(180), endAngle: .degrees(0), clockwise: false)
            g.stroke(a, with: .color(Scribble.yellow), style: StrokeStyle(lineWidth: 6, lineCap: .round))
        case (.watchingCloud, false):       // a drop of rain
            fill(Path(ellipseIn: CGRect(x: 48, y: 50, width: 7, height: 10)))
        case (.model, true):                // pins
            var p = Path()
            for k in 0..<3 {
                let v = CGFloat(22 + k * 10)
                p.move(to: CGPoint(x: v, y: 8)); p.addLine(to: CGPoint(x: v, y: 14))
                p.move(to: CGPoint(x: v, y: 50)); p.addLine(to: CGPoint(x: v, y: 56))
                p.move(to: CGPoint(x: 8, y: v)); p.addLine(to: CGPoint(x: 14, y: v))
                p.move(to: CGPoint(x: 50, y: v)); p.addLine(to: CGPoint(x: 56, y: v))
            }
            g.stroke(p, with: .color(Scribble.yellow), style: crayon)
        case (.tray, false):                // the lid
            fill(Path(roundedRect: CGRect(x: 8, y: 14, width: 48, height: 11), cornerRadius: 4))
        case (.wifi, true):                 // the signal over its head
            for r in [14.0, 22.0] as [CGFloat] {
                var a = Path(); a.addArc(center: CGPoint(x: 32, y: 40), radius: r + 8, startAngle: .degrees(-140), endAngle: .degrees(-40), clockwise: false)
                g.stroke(a, with: .color(Scribble.yellow), style: crayon)
            }
        case (.paperPlane, false):          // speed lines
            g.stroke(Path { $0.move(to: CGPoint(x: 4, y: 46)); $0.addLine(to: CGPoint(x: 14, y: 42)); $0.move(to: CGPoint(x: 8, y: 56)); $0.addLine(to: CGPoint(x: 18, y: 52)) },
                     with: .color(Scribble.yellow), style: crayon)
        default: break
        }
    }

    // MARK: Shapes (in a 64 box)

    static func body(_ icon: RisoIcon) -> Path {
        switch icon {
        case .priceTag:
            var p = Path()
            p.addLines([CGPoint(x: 10, y: 33), CGPoint(x: 22, y: 16), CGPoint(x: 58, y: 16), CGPoint(x: 58, y: 50), CGPoint(x: 22, y: 50)])
            p.closeSubpath()
            return p
        case .clock: return Path(ellipseIn: CGRect(x: 9, y: 12, width: 46, height: 44))
        case .privacy: return Path(roundedRect: CGRect(x: 12, y: 26, width: 40, height: 32), cornerRadius: 9)
        case .watchingCloud:
            var p = Path()
            for (x, y, r) in [(20.0, 38.0, 13.0), (34.0, 28.0, 16.0), (47.0, 38.0, 12.0), (32.0, 42.0, 14.0)] as [(CGFloat, CGFloat, CGFloat)] {
                p.addEllipse(in: CGRect(x: x - r, y: y - r, width: r * 2, height: r * 2))
            }
            return p
        case .model: return Path(roundedRect: CGRect(x: 13, y: 13, width: 38, height: 38), cornerRadius: 7)
        case .tray: return Path(roundedRect: CGRect(x: 11, y: 24, width: 42, height: 32), cornerRadius: 5)
        case .wifi: return Path(ellipseIn: CGRect(x: 16, y: 30, width: 32, height: 30))
        case .paperPlane:
            var p = Path()
            p.addLines([CGPoint(x: 60, y: 10), CGPoint(x: 6, y: 34), CGPoint(x: 26, y: 40), CGPoint(x: 34, y: 58)])
            p.closeSubpath()
            return p
        default: return Path(ellipseIn: CGRect(x: 10, y: 10, width: 44, height: 44))
        }
    }

    /// Where its eyes sit: their middle, the gap between them, their radius.
    static func eyes(_ icon: RisoIcon) -> (at: CGPoint, gap: CGFloat, r: CGFloat) {
        switch icon {
        case .priceTag: (CGPoint(x: 42, y: 33), 13, 4.6)
        case .clock: (CGPoint(x: 32, y: 34), 15, 5.4)
        case .privacy: (CGPoint(x: 32, y: 42), 14, 5)
        case .watchingCloud: (CGPoint(x: 33, y: 35), 14, 5.6)
        case .model: (CGPoint(x: 32, y: 32), 14, 5)
        case .tray: (CGPoint(x: 32, y: 40), 14, 5)
        case .wifi: (CGPoint(x: 32, y: 45), 12, 4.6)
        case .paperPlane: (CGPoint(x: 32, y: 34), 11, 4.2)
        default: (CGPoint(x: 32, y: 32), 14, 5)
        }
    }


    /// Crayon fill: long looping strokes wandering across the box, seeded so they never move.
    static func scribble(in r: CGRect, seed: Int, loops: Int) -> Path {
        var rng = SeededRandom(seed: seed)
        var p = Path()
        var at = CGPoint(x: r.minX + rng.next() * r.width, y: r.minY + rng.next() * r.height)
        p.move(to: at)
        for _ in 0..<loops {
            let next = CGPoint(x: r.minX - 4 + rng.next() * (r.width + 8), y: r.minY - 4 + rng.next() * (r.height + 8))
            let ctl = CGPoint(x: (at.x + next.x) / 2 + (rng.next() - 0.5) * r.width * 0.9,
                              y: (at.y + next.y) / 2 + (rng.next() - 0.5) * r.height * 0.9)
            p.addQuadCurve(to: next, control: ctl)
            at = next
        }
        return p
    }

    /// The edge gone over twice with a crayon: two slightly different passes.
    static func rough(_ shape: Path, seed: Int) -> Path {
        var out = Path()
        for pass in 0..<2 {
            var rng = SeededRandom(seed: seed + pass * 11)
            let dx = (rng.next() - 0.5) * 1.6, dy = (rng.next() - 0.5) * 1.6
            out.addPath(shape, transform: CGAffineTransform(translationX: dx, y: dy))
        }
        return out
    }
}
