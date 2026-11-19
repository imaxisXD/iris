import AppKit
import SwiftUI

/* ─────────────────────────────────────────────────────────
 * LOOKING FOR UPDATES: A LETTER FROM GITHUB
 *
 * The butterfly carries the question. Everything is posed from
 * the time since the click (`started`) and since the answer
 * (`answer.at`), so Settings and the slip land on the same frame.
 *
 * CLICK
 *     0.00 s  the bud grows up out of the ground, butterfly on it
 *     0.25 s  butterfly lifts off, loops over the heading and flies
 *             out past the edge (0.9 s), a dotted path behind it
 *     0.12 s  "Looking…" is written in (AnswerWords)
 * LOOP 1.4 s (at least once; the answer waits for it to come round)
 *             the bud sways, the dots pulse out towards GitHub
 * ANSWER
 *    +0.00 s  the path fades; the butterfly flies back in with a
 *             folded letter hanging under it (0.65 s)
 *    +0.65 s  the letter drops by the heading, grows, settles a
 *             few degrees off straight; "Looking…" wipes off
 *    +0.65 s  the butterfly hops back to the bud
 *    +0.95 s  the letter unfolds on its crease (a spring)
 *    +0.90 s  the answer is written in
 *    +1.15 s  the line under it rises in; the bud opens
 *    +1.25 s  the button becomes Check again / Try again / …
 * SETTLED     the letter and the bloom stay; the wings beat slowly
 * FAILED      back empty-handed, the bud stays shut, a red ring
 *             round "?" where the letter would be
 * REDUCE MOTION  no flight: the butterfly sits on the bud, the
 *             answer opens it and the letter fades in, open
 * ───────────────────────────────────────────────────────── */

/// The Updates art, in Settings (`.wide`, with the letter beside the bud) and on the slip's
/// Updates page (`.compact`, a small note). The flight runs past its frame, over the button and
/// off the edge; it never takes a click.
struct UpdateCheckScene: View {
    /// `resting`: the butterfly sits on the shut bud (Settings while an update builds).
    enum Outcome: Equatable { case upToDate, changes(Int), failed, resting }

    struct Answer: Equatable {
        let outcome: Outcome
        /// When the loop came round and the ending started.
        let at: Date
        /// What the letter says (Settings). The slip's small note needs only the outcome.
        var letter: Letter? = nil
    }

    /// The letter the butterfly brings back: a line in Iris's voice, then the facts.
    struct Letter: Equatable {
        struct Row: Equatable { let name: String; let value: String }
        let line: String
        let rows: [Row]
        var ticked = false
    }

    enum Layout { case wide, compact }

    /// The looking loop, and how long the ending plays after the answer.
    static let loop: TimeInterval = 1.4
    static let ending: TimeInterval = 1.7

    /// Every beat, in seconds: after the click (`lift`, `flyOut`) or after the answer.
    enum Beat {
        static let lift = 0.25, flyOut = 0.9
        static let back = 0.65, drop = 0.65, unfold = 0.95, ring = 0.9
        static let wipe = 0.65, write = 0.9, lead = 1.15, bloom = 1.15, button = 1.25
    }

    let started: Date
    var answer: Answer? = nil
    var layout: Layout = .compact
    /// Snapshots only: draw this moment.
    var frozenAt: Date? = nil

    /// Before anyone has asked: the bud shut, the butterfly resting on it.
    static var resting: UpdateCheckScene {
        UpdateCheckScene(started: .distantPast, answer: .init(outcome: .resting, at: .distantPast))
    }

    /// How long to wait, after `elapsed` seconds of looking, for the loop to come round: at
    /// least one whole loop, then to the end of the current one.
    static func wait(after elapsed: TimeInterval) -> TimeInterval {
        max(1, ceil(elapsed / loop)) * loop - elapsed
    }

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let stage = Stage.of(layout)
        // Landed and at rest: only the wings' slow beat is left, which thirty frames a second
        // draw as well as a hundred and twenty.
        let settled = answer.map { Date().timeIntervalSince($0.at) > Self.ending } ?? false
        TimelineView(.animation(minimumInterval: settled ? 1 / 30 : nil, paused: frozenAt != nil)) { timeline in
            scene(at: frozenAt ?? timeline.date, stage)
        }
        .frame(width: stage.size.width, height: stage.size.height, alignment: .topLeading)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    // MARK: Where things go

    private struct Stage: Equatable {
        let size: CGSize
        /// The bud's root, on the ground.
        let root: CGPoint
        let bud: CGFloat
        /// The letter where it lands (centre) and how big it opens.
        let letterAt: CGPoint
        let letter: CGSize
        /// How small it is while carried.
        let carried: CGFloat
        /// How far past the frame's right edge the butterfly goes (over the button, off the page).
        let reach: CGFloat
        /// Out to GitHub, then back with the answer; the last point is where the letter drops.
        let out: [CGPoint]
        let back: [CGPoint]
        let outDots: Int
        let backDots: Int
        /// The printed sky round the flower, fading into the paper at its edges.
        let sky: CGRect
        var perch: CGPoint { CGPoint(x: root.x + 4, y: root.y - bud) }

        static func of(_ layout: Layout) -> Stage { layout == .wide ? wide : compact }

        static let wide = Stage(
            size: CGSize(width: 222, height: 100), root: CGPoint(x: 188, y: 100), bud: 50,
            letterAt: CGPoint(x: 78, y: 54), letter: CGSize(width: 156, height: 80), carried: 0.3, reach: 230,
            out: [CGPoint(x: 194, y: 50), CGPoint(x: 150, y: 24), CGPoint(x: 96, y: 10), CGPoint(x: 122, y: -10),
                  CGPoint(x: 206, y: -6), CGPoint(x: 290, y: -16), CGPoint(x: 444, y: -2)],
            back: [CGPoint(x: 444, y: 70), CGPoint(x: 340, y: 86), CGPoint(x: 250, y: 66), CGPoint(x: 146, y: 20),
                   CGPoint(x: 80, y: 8)],
            outDots: 46, backDots: 30, sky: CGRect(x: 116, y: 6, width: 134, height: 100))

        static let compact = Stage(
            size: CGSize(width: 116, height: 76), root: CGPoint(x: 94, y: 76), bud: 46,
            letterAt: CGPoint(x: 36, y: 46), letter: CGSize(width: 58, height: 42), carried: 0.5, reach: 50,
            out: [CGPoint(x: 98, y: 30), CGPoint(x: 70, y: 10), CGPoint(x: 38, y: 6), CGPoint(x: 52, y: -8),
                  CGPoint(x: 104, y: -8), CGPoint(x: 166, y: 4)],
            back: [CGPoint(x: 166, y: 54), CGPoint(x: 114, y: 60), CGPoint(x: 70, y: 26), CGPoint(x: 38, y: 14)],
            outDots: 22, backDots: 14, sky: CGRect(x: 48, y: 8, width: 90, height: 72))
    }

    // MARK: The scene at one moment

    private func scene(at now: Date, _ st: Stage) -> some View {
        let t = now.timeIntervalSince(started)
        let a: TimeInterval? = answer.map { now.timeIntervalSince($0.at) }
        let outcome = answer?.outcome
        let resting = outcome == .resting
        let failed = outcome == .failed
        let still = reduceMotion || resting
        let p = (t / Self.loop).truncatingRemainder(dividingBy: 1)

        // The bud grows up on the click (Settings has no scene at rest), then opens on a good answer.
        let grow = still || answer != nil && t > 1 ? 1 : Ease.spring(seg(t, 0, 0.5), bounce: 0.2)
        let open = a.map { failed || resting ? 0 : (reduceMotion ? min(1, max(0, $0 / 0.3)) : Ease.out(seg($0, Beat.bloom, 0.45))) } ?? 0
        let sway: Double = still ? 0 : a.map { sin(seg($0, Beat.bloom, 0.45) * .pi) * 2 } ?? sin(p * 2 * .pi) * 3

        // The butterfly.
        let flapping = 0.3 + 0.7 * abs(sin(now.timeIntervalSinceReferenceDate * 2 * .pi * 7))
        let restingWings = 0.45 + 0.2 * max(0, sin(now.timeIntervalSinceReferenceDate * 2.4))
        let perch = CGPoint(x: st.perch.x, y: st.perch.y - CGFloat(open) * 4)
        var fly = perch, heading = 0.0, flap = restingWings, shown = 1.0, carrying = false
        if !still {
            if let a {
                if a < Beat.back {
                    let k = Ease.out(seg(a, 0, Beat.back))
                    fly = along(st.back, k)
                    heading = facing(fly, along(st.back, k + 0.01)) * (1 - k * 0.6)
                    flap = flapping; carrying = !failed
                    shown = seg(k, 0, 0.08)
                } else {
                    let k = Ease.inOut(seg(a, Beat.back, 0.5)), from = st.back[st.back.count - 1]
                    fly = CGPoint(x: lerp(from.x, perch.x, k), y: lerp(from.y, perch.y, k) - CGFloat(sin(k * .pi) * 14))
                    heading = k < 1 ? 20 * (1 - k) : 0
                    flap = k < 1 ? flapping : restingWings
                }
            } else if t > Beat.lift {
                let k = Ease.inOut(seg(t, Beat.lift, Beat.flyOut))
                fly = along(st.out, k)
                heading = facing(fly, along(st.out, k + 0.01))
                flap = flapping
                shown = 1 - seg(k, 0.86, 0.14)
            }
        }

        // The dotted path: drawn behind it on the way out, pulsing while it's away, gone on the answer.
        let outDrawn = still ? 0 : (a == nil ? Ease.inOut(seg(t, Beat.lift, Beat.flyOut)) : 1)
        let outFade = a.map { 1 - seg($0, 0, 0.5) } ?? 1
        let pulsing = a == nil && t > Beat.lift + Beat.flyOut
        let backDrawn = still ? 0 : a.map { Ease.out(seg($0, 0, Beat.back)) * (1 - seg($0, Self.ending, 0)) } ?? 0

        // The sky rises with the bud. A good answer sends the cloud off and brings the sun up
        // behind the bloom; a failed one leaves the cloud where it is.
        let good = a != nil && !failed && !resting
        let clears = a.map { good ? (reduceMotion ? 1 : Ease.inOut(seg($0, Beat.bloom - 0.35, 0.9))) : 0 } ?? 0
        let sunUp = a.map { good ? (reduceMotion ? 1 : Ease.out(seg($0, Beat.bloom, 0.8))) : 0 } ?? 0
        let drift = still ? 0 : sin(now.timeIntervalSinceReferenceDate * 0.7) * 3

        return ZStack(alignment: .topLeading) {
            sky(st, rise: grow, clears: clears, sunUp: sunUp, drift: drift)

            Canvas { ctx, _ in
                ctx.translateBy(x: 10, y: 30)
                let ink = GraphicsContext.Shading.color(DS.Ink.i12)
                for i in 0..<st.outDots {
                    let k = (Double(i) + 0.5) / Double(st.outDots)
                    guard k < outDrawn else { break }
                    let wave = pulsing ? 0.25 + 0.75 * pow((1 + sin((k - p) * 4 * .pi)) / 2, 2) : 0.75
                    let q = along(st.out, k)
                    ctx.opacity = wave * outFade * 0.75
                    ctx.fill(Path(ellipseIn: CGRect(x: q.x - 1.2, y: q.y - 1.2, width: 2.4, height: 2.4)), with: ink)
                }
                for i in 0..<st.backDots {
                    let k = (Double(i) + 0.5) / Double(st.backDots)
                    guard k < backDrawn else { break }
                    let q = along(st.back, k)
                    ctx.opacity = 0.3 * (a.map { 1 - seg($0, 1.2, 0.5) } ?? 1)
                    ctx.fill(Path(ellipseIn: CGRect(x: q.x - 1, y: q.y - 1, width: 2, height: 2)), with: ink)
                }
            }
            .frame(width: st.size.width + st.reach + 20, height: st.size.height + 40)
            .offset(x: -10, y: -30)

            // The flower: a bud swaying from the root, opening when the answer is good.
            ZStack {
                Image(nsImage: Sprites.bud).resizable().opacity(1 - open)
                Image(nsImage: Sprites.flower).resizable().opacity(open)
                    .scaleEffect(0.85 + 0.15 * overshoot(open), anchor: .bottom)
            }
            .frame(width: st.bud, height: st.bud)
            .scaleEffect(grow, anchor: .bottom)
            .rotationEffect(.degrees(sway), anchor: .bottom)
            .position(x: st.root.x, y: st.root.y - st.bud / 2)

            if failed, let a { ring(a, st) }

            if let answer, let a, !failed, !resting {
                letter(answer, a: a, st: st, carriedAt: fly, carrying: carrying)
            }

            Image(nsImage: Sprites.butterfly).interpolation(.none).resizable()
                .frame(width: st == .wide ? 22 : 20, height: st == .wide ? 22 : 20)
                .scaleEffect(x: flap, y: 1)
                .rotationEffect(.radians(heading))
                .scaleEffect(still || a != nil ? 1 : (t < Beat.lift ? grow : 1), anchor: .bottom)
                .opacity(shown)
                .position(fly)
        }
        .frame(width: st.size.width, height: st.size.height, alignment: .topLeading)
    }

    // MARK: The sky

    /// A pale cloud with an ink edge, so it reads on the blue.
    @MainActor private static let cloud = Sprites.skyCloud(.puffy, Sprites.Blue.mist, seed: 57)

    private func sky(_ st: Stage, rise: Double, clears: Double, sunUp: Double, drift: Double) -> some View {
        let w = st.sky.width, h = st.sky.height
        let sun = min(w, h) * 0.26, cloud = min(w, h) * 0.5
        return ZStack(alignment: .topLeading) {
            ScreenPlate(ink: .sky, seed: 23)
            Image(nsImage: Sprites.sun).interpolation(.none).resizable()
                .frame(width: sun, height: sun)
                .position(x: w * 0.42, y: h * 0.66 - CGFloat(sunUp) * h * 0.32)
                .opacity(sunUp > 0 ? 1 : 0)
            Image(nsImage: Self.cloud).interpolation(.none).resizable()
                .frame(width: cloud, height: cloud)
                .position(x: w * 0.47 + CGFloat(drift) + CGFloat(clears) * w * 0.35, y: h * 0.32)
                .opacity(1 - clears)
        }
        .frame(width: w, height: h)
        // No edge: the ink thins out from round the flower into the paper, like a sky fading.
        .mask(EllipticalGradient(stops: [.init(color: .black, location: 0), .init(color: .black.opacity(0.8), location: 0.3),
                                         .init(color: .black.opacity(0.4), location: 0.62), .init(color: .black.opacity(0.12), location: 0.85),
                                         .init(color: .clear, location: 1)],
                                 center: UnitPoint(x: 0.54, y: 0.5), startRadiusFraction: 0, endRadiusFraction: 0.5))
        .scaleEffect(CGFloat(rise), anchor: .bottom)
        .position(x: st.sky.midX, y: st.sky.midY)
    }

    // MARK: The letter

    /// Hanging under the butterfly, folded; dropped by the heading; opened on its crease.
    @ViewBuilder
    private func letter(_ answer: Answer, a: TimeInterval, st: Stage, carriedAt: CGPoint, carrying: Bool) -> some View {
        let w = st.letter.width, h = st.letter.height
        let landed = st.back[st.back.count - 1]
        // Where it is and how big.
        let (centre, scale, tilt): (CGPoint, CGFloat, Double) = {
            if reduceMotion { return (st.letterAt, 1, -3) }
            if a < Beat.drop {
                return (CGPoint(x: carriedAt.x, y: carriedAt.y + 15), st.carried, sin(a * 11) * 6)
            }
            let k = seg(a, Beat.drop, 0.3)
            let c = CGPoint(x: lerp(landed.x, st.letterAt.x, k), y: lerp(landed.y + 15, st.letterAt.y, Ease.in(k)))
            return (c, lerp(st.carried, 1, Ease.out(k)), mix(10, -3, Ease.spring(seg(a, Beat.drop, 0.6), bounce: 0.35)))
        }()
        // 180°: the top half folded down over the bottom; 0°: open.
        let unfold = reduceMotion ? 1 : (a < Beat.unfold ? 0 : Ease.spring(seg(a, Beat.unfold, 0.52), bounce: 0.22))
        let fold = 180 * (1 - unfold)
        // Folded, only the bottom half shows: keep what shows centred under the butterfly.
        let lift = CGFloat(1 - min(1, unfold)) * -h / 4 * scale
        let fade = reduceMotion ? min(1, max(0, a / 0.3)) : 1

        LetterView(letter: answer.letter, outcome: answer.outcome, date: answer.at, size: st.letter,
                   wide: st == .wide, fold: fold, tick: reduceMotion ? 1 : Ease.out(seg(a, Beat.unfold + 0.3, 0.3)))
            .frame(width: w, height: h)
            .scaleEffect(scale)
            .rotationEffect(.degrees(tilt))
            .opacity(fade)
            .position(x: centre.x, y: centre.y + lift)
    }

    /// Back empty-handed: a red ring round "?" where the letter would have landed.
    private func ring(_ a: TimeInterval, _ st: Stage) -> some View {
        let d: CGFloat = st == .wide ? 36 : 28
        return ZStack {
            BrushRing(rgb: Riso.red, seed: 19, drawn: CGFloat(reduceMotion ? 1 : Ease.out(seg(a, Beat.ring, 0.4))))
            Text("?").font(DS.Font.brush(d * 0.62)).foregroundStyle(DS.Hue.redText)
                .opacity(reduceMotion ? 1 : seg(a, Beat.ring + 0.25, 0.2))
        }
        .frame(width: d, height: d)
        .rotationEffect(.degrees(-6))
        .position(x: st == .wide ? st.letterAt.x + 40 : st.letterAt.x + 6, y: st.letterAt.y - 4)
    }

    // MARK: Paths and easing

    /// A point a fraction `k` along a smooth curve through `pts` (Catmull-Rom).
    private func along(_ pts: [CGPoint], _ k: Double) -> CGPoint {
        let n = pts.count - 1, x = min(1, max(0, k)) * Double(n)
        let i = min(n - 1, Int(x)), u = CGFloat(x - Double(i))
        let p0 = pts[max(0, i - 1)], p1 = pts[i], p2 = pts[i + 1], p3 = pts[min(n, i + 2)]
        func f(_ a: CGFloat, _ b: CGFloat, _ c: CGFloat, _ d: CGFloat) -> CGFloat {
            0.5 * (2 * b + (-a + c) * u + (2 * a - 5 * b + 4 * c - d) * u * u + (-a + 3 * b - 3 * c + d) * u * u * u)
        }
        return CGPoint(x: f(p0.x, p1.x, p2.x, p3.x), y: f(p0.y, p1.y, p2.y, p3.y))
    }

    /// The butterfly's sprite faces up: turn it to face along its path.
    private func facing(_ from: CGPoint, _ to: CGPoint) -> Double { atan2(to.y - from.y, to.x - from.x) + .pi / 2 }
    private func overshoot(_ x: Double) -> Double { x >= 1 ? 1 : x + sin(x * .pi) * 0.12 }
}

/// 0…1 through a beat that starts at `start` and lasts `length` seconds.
private func seg(_ t: Double, _ start: Double, _ length: Double) -> Double {
    length <= 0 ? (t >= start ? 1 : 0) : min(1, max(0, (t - start) / length))
}
private func lerp(_ a: CGFloat, _ b: CGFloat, _ k: Double) -> CGFloat { a + (b - a) * CGFloat(k) }
private func mix(_ a: Double, _ b: Double, _ k: Double) -> Double { a + (b - a) * k }

/// The update check's easing, the same in the scene and its words.
enum Ease {
    static func out(_ x: Double) -> Double { 1 - pow(1 - x, 3) }
    static func `in`(_ x: Double) -> Double { x * x * x }
    static func inOut(_ x: Double) -> Double { x < 0.5 ? 4 * x * x * x : 1 - pow(-2 * x + 2, 3) / 2 }
    /// A damped spring from 0 to 1 over normalised time `x`; `bounce` 0…0.6 goes past and back.
    static func spring(_ x: Double, bounce: Double) -> Double {
        guard x > 0 else { return 0 }
        guard x < 1 else { return 1 }
        let zeta = 1 - bounce, w = 9.0, wd = w * sqrt(1 - zeta * zeta)
        let v = 1 - exp(-zeta * w * x) * (cos(wd * x) + zeta * w / wd * sin(wd * x))
        // Land exactly on 1 at the end so a settled frame never drifts.
        return v + (1 - (1 - exp(-zeta * w) * (cos(wd) + zeta * w / wd * sin(wd)))) * x
    }
}

// MARK: - The letter itself

/// A sheet of paper folded once across the middle. The top half swings on the crease (`fold`
/// 180° folded down, 0° open): its back, with a pink ✻ seal holding it shut, shows while folded.
private struct LetterView: View {
    let letter: UpdateCheckScene.Letter?
    let outcome: UpdateCheckScene.Outcome
    let date: Date
    let size: CGSize
    let wide: Bool
    let fold: Double
    /// The pink tick, drawn in 0…1.
    let tick: Double

    var body: some View {
        let half = size.height / 2
        ZStack(alignment: .top) {
            // The bottom half, with its crema shadow plate printed off register.
            sheet(seed: 61) { bottom }
                .frame(width: size.width, height: half)
                .offset(y: half)
            // The top half, swinging on the crease.
            ZStack {
                sheet(seed: 62) { top }.opacity(fold < 90 ? 1 : 0)
                sheet(seed: 63) { seal }.scaleEffect(x: 1, y: -1).opacity(fold < 90 ? 0 : 1)
            }
            .frame(width: size.width, height: half)
            .rotation3DEffect(.degrees(fold), axis: (x: 1, y: 0, z: 0), anchor: .bottom, perspective: 0.35)
        }
        .frame(width: size.width, height: size.height, alignment: .top)
    }

    private func sheet<Content: View>(seed: Int, @ViewBuilder _ content: () -> Content) -> some View {
        ZStack(alignment: .topLeading) {
            HandRect(seed: seed, wobble: 0.35, closed: true).printedInk(Riso.crema, seed: seed).offset(x: 1.5, y: 1.5)
            HandRect(seed: seed, wobble: 0.35, closed: true).fill(DS.Surface.top)
            content()
            HandRect(seed: seed + 9, wobble: 0.35).stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 1.1, lineCap: .round))
        }
    }

    /// The front of the top half: who it's from, then Iris's line.
    @ViewBuilder private var top: some View {
        if wide, let letter {
            VStack(alignment: .leading, spacing: 1) {
                Text("FROM GITHUB · \(Self.day.string(from: date).uppercased())")
                    .font(DS.Font.martian(8.5)).tracking(0.5).foregroundStyle(DS.Ink.i10).lineLimit(1)
                Text(letter.line).font(DS.Font.brush(17)).foregroundStyle(DS.Ink.i12).lineLimit(1).minimumScaleFactor(0.8)
            }
            .padding(.horizontal, 10).padding(.top, 6)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .overlay(alignment: .bottom) {
                HandLine(seed: 64, wobble: 0.4).stroke(DS.Ink.i8, lineWidth: 0.8).frame(height: 1).padding(.horizontal, 4)
            }
        } else {
            // The small note on the slip: a few pen lines where writing would be.
            VStack(alignment: .leading, spacing: 5) {
                ForEach(0..<2, id: \.self) { i in
                    HandLine(seed: 70 + i, wobble: 0.5).stroke(DS.Ink.i8, lineWidth: 1).frame(width: size.width * (i == 0 ? 0.6 : 0.42), height: 1)
                }
            }
            .padding(.leading, 8).padding(.top, 7)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
    }

    /// The bottom half: the facts and the tick (Settings), or the tick or the count (the slip).
    @ViewBuilder private var bottom: some View {
        if wide, let letter {
            HStack(alignment: .center, spacing: 6) {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(letter.rows, id: \.name) { row in
                        HStack(spacing: 0) {
                            Text(row.name).frame(width: 50, alignment: .leading).foregroundStyle(DS.Ink.i10)
                            Text(row.value).foregroundStyle(DS.Ink.i12)
                        }
                        .font(DS.Font.martian(9.5)).lineLimit(1)
                    }
                }
                Spacer(minLength: 0)
                if letter.ticked { penTick.frame(width: 20, height: 15) }
            }
            .padding(.horizontal, 10)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            Group {
                switch outcome {
                case .changes(let count):
                    Text("\(count)").font(DS.Font.brush(17)).foregroundStyle(DS.Hue.pinkText)
                default:
                    penTick.frame(width: 18, height: 13)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private var penTick: some View {
        HandTick().trim(from: 0, to: CGFloat(tick))
            .stroke(DS.Hue.pinkText, style: StrokeStyle(lineWidth: 2.4, lineCap: .round, lineJoin: .round))
    }

    /// The back of the top half: a printed pink sticker with a ✻ holding the fold shut.
    private var seal: some View {
        ZStack {
            HandRect(seed: 65, wobble: 0.5, closed: true).printedInk(Riso.platePink, seed: 65)
            Text("✻").font(DS.Font.martian(wide ? 11 : 8)).foregroundStyle(DS.Surface.top)
        }
        .frame(width: wide ? 22 : 13, height: wide ? 17 : 10)
        .rotationEffect(.degrees(-4))
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private static let day: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_GB")
        f.dateFormat = "d MMM"
        return f
    }()
}

// MARK: - Words that change with the answer

/// A clock that ticks every frame until `until`, then stops: the update check's words and
/// button only move while the answer lands.
struct AnswerClock: TimelineSchedule {
    let until: Date
    func entries(from start: Date, mode: TimelineScheduleMode) -> AnyIterator<Date> {
        var next = start, done = false
        return AnyIterator {
            guard !done else { return nil }
            if next >= until { done = true; return until.addingTimeInterval(0.01) }
            defer { next = next.addingTimeInterval(1.0 / 60) }
            return next
        }
    }
}

/// Words that change as the letter arrives instead of jumping: the old ones wipe off, the new
/// ones are written in left to right like the brush putting them down (`.written`, the title)
/// or rise into place (`.rising`, the line under it). Both stay laid out, so nothing under them
/// moves while they change. Reduce Motion: a fade.
struct AnswerWords: View {
    enum Style { case written, rising }

    let before: String?
    let after: String
    /// When the change began (the answer, or the click); nil: the new words, still.
    let at: Date?
    var style: Style = .written
    let font: Font
    var ink: Color = DS.Ink.i12
    var beforeInk: Color = DS.Ink.i12
    /// Seconds after `at`: when the old words go and the new ones come.
    var goes: TimeInterval = UpdateCheckScene.Beat.wipe
    var comes: TimeInterval = UpdateCheckScene.Beat.write
    var frozenAt: Date? = nil

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        if let at, frozenAt == nil, Date().timeIntervalSince(at) < comes + 0.6 {
            TimelineView(AnswerClock(until: at.addingTimeInterval(comes + 0.6))) { timeline in
                words(timeline.date.timeIntervalSince(at))
            }
        } else {
            words(at.map { (frozenAt ?? Date()).timeIntervalSince($0) } ?? .infinity)
        }
    }

    private func words(_ a: TimeInterval) -> some View {
        let out = reduceMotion ? seg(a, goes, 0.2) : Ease.in(seg(a, goes, 0.2))
        let into = reduceMotion ? seg(a, comes, 0.3) : Ease.out(seg(a, comes, style == .written ? 0.48 : 0.3))
        return ZStack(alignment: .topLeading) {
            if let before {
                text(before, beforeInk).modifier(Reveal(style: style, k: 1 - out, leaving: true, still: reduceMotion))
            }
            text(after, ink).modifier(Reveal(style: style, k: into, leaving: false, still: reduceMotion))
        }
    }

    private func text(_ s: String, _ ink: Color) -> some View {
        Text(s).font(font).foregroundStyle(ink).fixedSize(horizontal: false, vertical: true)
    }

    /// How much of the words shows, 0…1.
    private struct Reveal: ViewModifier {
        let style: Style
        let k: Double
        let leaving: Bool
        let still: Bool
        func body(content: Content) -> some View {
            if still || style == .rising {
                content.opacity(k).offset(y: still || leaving ? 0 : CGFloat(1 - k) * 6)
            } else {
                // A soft edge travels left to right: in, it uncovers; out, it rubs away.
                let x = -0.12 + 1.24 * k
                content.mask {
                    LinearGradient(stops: leaving
                                   ? [.init(color: .clear, location: max(0, 1 - x - 0.1)), .init(color: .black, location: min(1, max(0.0001, 1 - x)))]
                                   : [.init(color: .black, location: max(0, x)), .init(color: .clear, location: min(1, max(0.0001, x + 0.1)))],
                                   startPoint: leaving ? .trailing : .leading, endPoint: leaving ? .leading : .trailing)
                }
            }
        }
    }
}

/// Shows `before` until `delay` seconds after the answer, then `after` (the heading's button
/// waits for the letter to open), crossing over with the page's ease.
struct AfterAnswer<Before: View, After: View>: View {
    let at: Date
    let delay: TimeInterval
    @ViewBuilder let before: Before
    @ViewBuilder let after: After

    @State private var arrived = false

    var body: some View {
        let when = at.addingTimeInterval(delay)
        VStack(alignment: .leading, spacing: 0) {
            if arrived || Date() >= when { after.transition(.opacity) } else { before.transition(.opacity) }
        }
        .animation(DS.easeOut, value: arrived)
        .task(id: when) {
            let wait = when.timeIntervalSinceNow
            if wait > 0 {
                arrived = false
                try? await Task.sleep(for: .seconds(wait))
                guard !Task.isCancelled else { return }
            }
            arrived = true
        }
    }
}
