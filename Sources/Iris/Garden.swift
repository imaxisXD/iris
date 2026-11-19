import AppKit
import IrisCore
import IrisEngine
import SwiftUI

/// The setup screen's flower, telling the setup story as it goes:
///
///   bud       waiting for permission: a closed bud sways
///   allowed   step 1 done: it opens, the first butterfly flutters in
///   watering  downloading: a riso cloud rains on it, harder as the download goes
///   warming   loading into memory: the cloud leaves, a sun rises behind
///   ready     model ready: a second butterfly arrives
///   tried     the practice box was fixed: two more, and a shower of petals
///
/// `growth` tells a different story, for an update: a small sprout under the rain cloud that
/// grows into the full flower as the build goes, butterflies only once it's grown.
///
/// Under Reduce Motion nothing travels: butterflies sit still, no rain, no sway.
extension GardenStage {
    var butterflies: Int {
        switch self {
        case .bud: 0
        case .allowed, .watering, .warming: 1
        case .ready: 2
        case .tried: 4
        }
    }
}

struct GardenView: View {
    typealias Stage = GardenStage

    let stage: Stage
    /// Download progress, for how hard it rains.
    var progress: Double = 0
    /// 0...1 while an update builds: the flower grows from a sprout to full size.
    var growth: Double? = nil
    /// Snapshots, and windows on the test display (`OnDisplay`): draw one moment, every arrival
    /// already settled, the moment snapshots draw.
    var frozenAt: Double? = OnDisplay.isOn ? 1000 : nil

    static let size = CGSize(width: 190, height: 190)

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var arrivals: [Int: Double] = [:]
    @State private var bloomedAt: Double?
    @State private var sunAt: Double?
    @State private var triedAt: Double?
    /// Going back (the model freed): when each butterfly left, the sun set, the bloom closed.
    @State private var departures: [Int: Double] = [:]
    @State private var sunSetAt: Double?
    @State private var closedAt: Double?
    /// While growing: when each small butterfly set off.
    @State private var swarm: [Int: Double] = [:]

    var body: some View {
        TimelineView(.animation(paused: reduceMotion || frozenAt != nil)) { timeline in
            let t = frozenAt ?? timeline.date.timeIntervalSinceReferenceDate
            scene(at: t)
        }
        .frame(width: Self.size.width, height: Self.size.height)
        .onAppear { settle(animated: false) }
        .onChange(of: stage) { _, _ in settle(animated: true) }
        .onChange(of: growth) { _, _ in settle(animated: true) }
        .accessibilityHidden(true)
    }

    // MARK: Story beats

    /// Records when each beat started, so it can play once from there. On first appearance
    /// everything already true counts as long settled (no replay when the window reopens).
    private func settle(animated: Bool) {
        let now = animated && frozenAt == nil ? Date.timeIntervalSinceReferenceDate : -1000
        for i in 0..<stage.butterflies where arrivals[i] == nil {
            arrivals[i] = now + Double(i % 2) * 0.5
            departures[i] = nil
        }
        if stage >= .allowed, bloomedAt == nil { bloomedAt = now; closedAt = nil }
        if stage >= .warming, sunAt == nil { sunAt = now; sunSetAt = nil }
        // Going back, as when the model is freed: the story plays in reverse, a moment each.
        for i in Array(arrivals.keys) where i >= stage.butterflies {
            arrivals[i] = nil
            departures[i] = now + Double(i % 2) * 0.3
        }
        if stage < .warming, sunAt != nil { sunAt = nil; sunSetAt = now }
        if stage < .allowed, bloomedAt != nil { bloomedAt = nil; closedAt = now }
        if stage == .tried, triedAt == nil { triedAt = frozenAt.map { $0 - 0.9 } ?? now }
        // Growing: more small butterflies as the flower opens, each setting off a moment apart.
        for i in 0..<swarmCount where swarm[i] == nil { swarm[i] = now + hash(i, 300) * 1.2 }
    }

    private func age(_ start: Double?, _ t: Double) -> Double? {
        guard let start else { return nil }
        return reduceMotion ? 1000 : t - start
    }

    // MARK: Scene

    /// The butterflies loop above and around the bloom, against the paper, not over the petals.
    private let flowerHead = CGPoint(x: 95, y: 58)

    private func scene(at t: Double) -> some View {
        let still = reduceMotion
        let bloomAge = age(bloomedAt, t)
        var open = bloomAge.map { ease(min(1, $0 / 0.9)) }
            ?? age(closedAt, t).map { 1 - ease(min(1, max(0, $0) / 0.9)) } ?? 0
        let sway = still ? 0 : sin(t * 1.3) * 2.2
        // Growing: a sprout (the bud, small) that opens halfway up and reaches full size at the end.
        var size: Double = 1
        if let growth {
            let g = min(1, max(0, growth))
            size = 0.36 + 0.64 * ease(g)
            open = min(1, max(0, (g - 0.35) / 0.4))
        }

        return ZStack(alignment: .topLeading) {
            // Sun, behind everything: rises once the model starts loading.
            // It sets again, sinking and fading, when the model is freed.
            if let sunAge = age(sunAt, t) {
                sun(rise: ease(min(1, sunAge / 1.4)), at: t)
            } else if let setAge = age(sunSetAt, t), setAge < 1.4 {
                sun(rise: 1 - ease(max(0, setAge) / 1.4), at: t)
            }

            // The flower: a bud that springs open, swaying from the root.
            ZStack {
                Image(nsImage: Sprites.bud).resizable().opacity(1 - open)
                Image(nsImage: Sprites.flower).resizable().opacity(open)
                    .scaleEffect(0.85 + 0.15 * overshoot(open), anchor: .bottom)
            }
            .frame(width: 128, height: 128)
            .scaleEffect(size, anchor: UnitPoint(x: 0.5, y: 0.92))
            .rotationEffect(.degrees(sway), anchor: .bottom)
            .position(x: 95, y: 124)

            // Watering: a cloud over the flower, rain falling harder as the download goes. While
            // growing it's a rainy sky instead: small clouds drifting, each with its own rain.
            if stage == .watering {
                let clouds = growth == nil ? [Cloud.single] : Cloud.sky
                ForEach(Array(clouds.enumerated()), id: \.offset) { index, cloud in
                    if cloud.rains { rain(at: t, from: cloud, salt: index * 50, flowerTop: 178 - 99 * size) }
                    Image(nsImage: cloud.image).interpolation(.none).resizable()
                        .frame(width: cloud.width, height: cloud.width)
                        .position(x: cloud.x(t, still: still), y: cloud.y)
                        .transition(.opacity)
                }
            }

            // Butterflies, each on its own loop around the flower head.
            ForEach(0..<(growth == nil ? stage.butterflies : 0), id: \.self) { i in
                butterfly(i, at: t)
            }
            // Leaving: off their loops and out over the top right, fading as they go.
            if growth == nil {
                ForEach(departures.filter { age($0.value, t).map { $0 < 1.6 } ?? false }.keys.sorted(), id: \.self) { i in
                    leaving(i, at: t)
                }
            }
            // Growing: small ones arrive in a bunch, each on a loop of its own.
            ForEach(0..<swarmCount, id: \.self) { i in
                smallButterfly(i, at: t, headY: 186 - 108 * size)
            }

            // Tried it: a shower of petals, once.
            if let triedAge = age(triedAt, t), triedAge < 3.2 {
                petals(age: triedAge)
            }
        }
        .frame(width: Self.size.width, height: Self.size.height, alignment: .topLeading)
    }

    /// A cloud over the garden, bobbing side to side on its own slow beat.
    /// A cloud over the garden: the bank drifts side to side as one, each cloud bobbing a
    /// touch on its own so they stay touching without moving in lockstep.
    private struct Cloud {
        let image: NSImage
        let x0, y, width: Double
        var drift = 8.0, speed = 0.22, bob = 1.5, phase = 0.0
        var rains = true
        func x(_ t: Double, still: Bool) -> Double {
            still ? x0 : x0 + sin(t * speed) * drift + sin(t * 0.9 + phase) * bob
        }
        /// The download's one cloud, over the flower.
        @MainActor static let single = Cloud(image: Sprites.cloud, x0: 98, y: 24, width: 50, drift: 6, speed: 0.8, bob: 0)
        /// An update's rainy sky, low and wide: a long pale bank behind, a puffy and a round
        /// cloud shoulder to shoulder in front, a wisp drifting above (no rain from the wisp).
        /// Listed back to front.
        @MainActor static let sky = [
            Cloud(image: Sprites.skyCloud(.wisp, Sprites.Blue.mist, edge: Sprites.Blue.sky, seed: 51), x0: 140, y: 14, width: 34,
                  phase: 0.4, rains: false),
            Cloud(image: Sprites.skyCloud(.long, Sprites.Blue.pale, density: 75, seed: 52), x0: 100, y: 26, width: 76, phase: 1.7),
            Cloud(image: Sprites.skyCloud(.puffy, Sprites.Blue.mid, seed: 53), x0: 74, y: 36, width: 44, phase: 3.1),
            Cloud(image: Sprites.skyCloud(.round, Sprites.Blue.dusk, seed: 54), x0: 120, y: 38, width: 40, phase: 4.6),
        ]
    }

    /// Rain from under a cloud. Every drop keeps its own pace and, on each fall, starts from a
    /// new spot under the cloud, so it never reads as a pattern; drops slant a touch with the
    /// wind and splash where they land: on the flower's head, or the ground beside it.
    private func rain(at t: Double, from cloud: Cloud, salt: Int, flowerTop: Double) -> some View {
        let drops = reduceMotion ? 0 : Int(Double(5 + Int(progress * 7)) * cloud.width / 50)
        let top = cloud.y + cloud.width * 0.2
        return ZStack(alignment: .topLeading) {
            ForEach(0..<drops, id: \.self) { j in
                let i = j + salt
                let speed = 0.75 + 0.6 * hash(i, -1)            // falls per second
                let cycle = t * speed + hash(i, -2) * 7
                let n = floor(cycle), fall = cycle - n
                let start = cloud.x(t - fall / speed, still: false)  // where the cloud was when it fell
                let x0 = start + (hash(i, Int(n)) - 0.5) * cloud.width * 0.6
                let landing = abs(x0 - 95) < 26 * max(0.5, (178 - flowerTop) / 99) ? flowerTop : 182
                let length = max(10, landing - top)
                let x = x0 + fall * 3
                let tall = 4 + hash(i, Int(n) + 9) * 3
                // Espresso drops: pink would vanish into the petals they land on.
                if fall < 0.9 {
                    Capsule().fill(DS.Hue.espresso).frame(width: 1.6, height: tall)
                        .rotationEffect(.degrees(-6))
                        .opacity(0.75 * min(1, fall * 8))
                        .position(x: x, y: top + fall * length)
                } else {
                    // The splash: two specks thrown out sideways.
                    let k = (fall - 0.9) / 0.1
                    ForEach([-1.0, 1.0], id: \.self) { side in
                        Capsule().fill(DS.Hue.espresso).frame(width: 2, height: 1.4)
                            .opacity(0.6 * (1 - k))
                            .position(x: x + side * (1 + k * 4), y: top + 0.9 * length - k * 2)
                    }
                }
            }
        }
    }

    /// While growing: none until the flower starts to open, then up to six.
    private var swarmCount: Int {
        guard let growth else { return 0 }
        return min(6, max(0, Int((growth - 0.3) / 0.7 * 7)))
    }

    /// A small butterfly for the update's swarm: in from one side, then a loop of its own
    /// around wherever the flower's head is now.
    @ViewBuilder private func smallButterfly(_ i: Int, at t: Double, headY: Double) -> some View {
        let rx = 26 + hash(i, 101) * 44, ry = 10 + hash(i, 102) * 18
        let speed = (0.7 + hash(i, 103) * 0.7) * (hash(i, 104) < 0.5 ? -1 : 1), phase = hash(i, 105) * 6.3
        let size = 15 + hash(i, 106) * 6
        if reduceMotion {
            Image(nsImage: Sprites.butterfly).interpolation(.none).resizable().frame(width: size, height: size)
                .position(x: 95 + (Double(i % 2) * 2 - 1) * (40 + Double(i) * 8), y: headY - 10 + Double(i) * 6)
        } else {
            let here = swarmPoint(t, rx: rx, ry: ry, speed: speed, phase: phase, headY: headY)
            let next = swarmPoint(t + 0.05, rx: rx, ry: ry, speed: speed, phase: phase, headY: headY)
            let heading = atan2(next.y - here.y, next.x - here.x) + .pi / 2
            let arrive = age(swarm[i], t).map { ease(min(1, max(0, $0) / 1.6)) } ?? 0
            let fromLeft = hash(i, 107) < 0.5
            // In from just inside the garden's edge, fading in: the garden sits at the paper's
            // edge, and nothing should fly in from off the paper.
            let entry = CGPoint(x: fromLeft ? 8 : Self.size.width - 8, y: 10 + hash(i, 108) * 60)
            let point = CGPoint(x: entry.x + (here.x - entry.x) * arrive, y: entry.y + (here.y - entry.y) * arrive)
            Image(nsImage: Sprites.butterfly).interpolation(.none).resizable()
                .frame(width: size, height: size)
                .scaleEffect(x: 0.28 + 0.72 * abs(sin(t * 17 + phase)), y: 1)
                .rotationEffect(.radians(arrive < 1 ? (fromLeft ? .pi / 2 : -.pi / 2) : heading))
                .opacity(swarm[i] == nil ? 0 : min(1, arrive * 3))
                .position(point)
        }
    }

    private func swarmPoint(_ t: Double, rx: Double, ry: Double, speed: Double, phase: Double, headY: Double) -> CGPoint {
        let a = t * speed + phase
        return CGPoint(x: 95 + rx * sin(a), y: headY - 6 + ry * sin(2 * a) * 0.6 - 6 * sin(a * 1.7))
    }

    /// A fixed random number in 0..<1 for a drop and a fall.
    private func hash(_ i: Int, _ n: Int) -> Double {
        let v = sin(Double(i) * 12.9898 + Double(n) * 78.233) * 43758.5453
        return v - floor(v)
    }

    private struct Loop { let rx, ry, speed, phase: Double }
    private let loops = [Loop(rx: 66, ry: 24, speed: 0.55, phase: 0), Loop(rx: 58, ry: 30, speed: 0.72, phase: 2.2),
                         Loop(rx: 72, ry: 20, speed: 0.46, phase: 4.1), Loop(rx: 50, ry: 26, speed: 0.84, phase: 1.1)]
    /// Where each butterfly rests under Reduce Motion: beside the flower, on the paper.
    private let perches = [CGPoint(x: 160, y: 50), CGPoint(x: 26, y: 96), CGPoint(x: 168, y: 118), CGPoint(x: 30, y: 30)]

    private func loopPoint(_ loop: Loop, _ t: Double) -> CGPoint {
        let a = t * loop.speed + loop.phase
        return CGPoint(x: flowerHead.x + loop.rx * sin(a),
                       y: flowerHead.y - 8 + loop.ry * sin(2 * a) * 0.6 - 8 * sin(a * 1.3))
    }

    @ViewBuilder private func butterfly(_ i: Int, at t: Double) -> some View {
        let loop = loops[i % loops.count]
        if reduceMotion {
            sprite(flap: 1, heading: 0).position(perches[i % perches.count])
        } else {
            let here = loopPoint(loop, t), next = loopPoint(loop, t + 0.05)
            let heading = atan2(next.y - here.y, next.x - here.x) + .pi / 2
            // Arrival: in from the top right, fading in just inside the edge (the garden can sit
            // at the paper's edge, and nothing flies in from off the paper), easing onto its loop.
            let arrive = age(arrivals[i], t).map { ease(min(1, max(0, $0) / 1.8)) } ?? 1
            let point = CGPoint(x: Self.exit.x + (here.x - Self.exit.x) * arrive, y: Self.exit.y + (here.y - Self.exit.y) * arrive)
            sprite(flap: 0.28 + 0.72 * abs(sin(t * 15 + loop.phase)), heading: arrive < 1 ? 3.6 : heading)
                .opacity(arrivals[i] == nil ? 0 : min(1, arrive * 3))
                .position(point)
        }
    }

    /// Where butterflies come in and go out: the top right, just inside the edge.
    private static let exit = CGPoint(x: size.width - 10, y: 8)

    @ViewBuilder private func leaving(_ i: Int, at t: Double) -> some View {
        let loop = loops[i % loops.count]
        let here = loopPoint(loop, t)
        let k = age(departures[i], t).map { ease(min(1, max(0, $0) / 1.6)) } ?? 1
        let point = CGPoint(x: here.x + (Self.exit.x - here.x) * k, y: here.y + (Self.exit.y - here.y) * k)
        let heading = atan2(Self.exit.y - here.y, Self.exit.x - here.x) + .pi / 2
        sprite(flap: 0.28 + 0.72 * abs(sin(t * 18 + loop.phase)), heading: k > 0.15 ? heading : 0)
            .opacity(1 - k)
            .position(point)
    }

    private func sun(rise: Double, at t: Double) -> some View {
        Image(nsImage: Sprites.sun).interpolation(.none).resizable()
            .frame(width: 58, height: 58)
            .rotationEffect(.degrees(reduceMotion ? 0 : t * 9))
            .opacity(rise)
            .position(x: 40, y: 66 + (1 - rise) * 40)
    }

    private func sprite(flap: Double, heading: Double) -> some View {
        Image(nsImage: Sprites.butterfly).interpolation(.none).resizable()
            .frame(width: 32, height: 32)
            .scaleEffect(x: flap, y: 1)
            .rotationEffect(.radians(heading))
    }

    private func petals(age: Double) -> some View {
        ZStack(alignment: .topLeading) {
            ForEach(0..<16, id: \.self) { i in
                let seed = Double(i) * 0.6180339
                let x0 = 20 + seed.truncatingRemainder(dividingBy: 1) * 140
                let delay = seed.truncatingRemainder(dividingBy: 0.5)
                let p = max(0, min(1, (age - delay) / 2.4))
                Ellipse().printedInk(i % 3 == 0 ? Riso.crema : Riso.pink, seed: 40 + i)
                    .frame(width: 7, height: 4)
                    .rotationEffect(.degrees(age * 220 + seed * 300))
                    .opacity(p == 0 || p == 1 ? 0 : 1 - p * 0.6)
                    .position(x: x0 + sin(age * 3 + seed * 6) * 10, y: -6 + p * 180)
            }
        }
    }

    // MARK: Curves

    private func ease(_ x: Double) -> Double { 1 - pow(1 - x, 3) }
    /// Past 1 and back, for the bloom's spring.
    private func overshoot(_ x: Double) -> Double { x >= 1 ? 1 : x + sin(x * .pi) * 0.12 }
}

/// The garden's riso prints, made once.
@MainActor
enum Sprites {
    static let flower = RisoArt.flower(points: 128)
    static let bud = RisoArt.flower(points: 128, open: false)

    /// Seen from above, wings open: pointed orange forewings, pale hindwings, an ink body.
    /// Orange, not pink, so it stands off the flower it visits.
    static let butterfly: NSImage = {
        func wing(_ points: [(CGFloat, CGFloat)], mirror: Bool) -> CGPath {
            let p = CGMutablePath()
            let pts = points.map { CGPoint(x: mirror ? 80 - $0.0 : $0.0, y: $0.1) }
            p.move(to: pts[0])
            p.addQuadCurve(to: pts[2], control: pts[1])
            p.addQuadCurve(to: pts[4], control: pts[3])
            p.closeSubpath()
            return p
        }
        let fore: [(CGFloat, CGFloat)] = [(38, 30), (14, 4), (4, 20), (10, 42), (38, 40)]
        let hind: [(CGFloat, CGFloat)] = [(38, 42), (12, 46), (18, 68), (30, 70), (38, 52)]
        func wings() -> [(CGPath, CGFloat)] {
            [(wing(fore, mirror: false), 45), (wing(fore, mirror: true), 45),
             (wing(hind, mirror: false), 175), (wing(hind, mirror: true), 175)]
        }
        return sprite(pixels: 36, seed: 21, colour: Riso.orange,
            shade: { ctx in for (w, _) in wings() { ctx.fill(w, 160) } },
            pink: { ctx in for (w, g) in wings() { ctx.fill(w, g) } },
            ink: { ctx in
                ctx.pen(3.2)
                for (w, _) in wings() { ctx.addPath(w); ctx.strokePath() }
                ctx.pen(6.5)
                ctx.move(to: CGPoint(x: 40, y: 26)); ctx.addLine(to: CGPoint(x: 40, y: 60)); ctx.strokePath()
                ctx.pen(2.6)
                ctx.move(to: CGPoint(x: 39, y: 24)); ctx.addQuadCurve(to: CGPoint(x: 31, y: 9), control: CGPoint(x: 37, y: 12)); ctx.strokePath()
                ctx.move(to: CGPoint(x: 41, y: 24)); ctx.addQuadCurve(to: CGPoint(x: 49, y: 9), control: CGPoint(x: 43, y: 12)); ctx.strokePath()
            })
    }()

    /// A puffy cloud, crema with an ink edge.
    static let cloud: NSImage = {
        func shape() -> CGPath { cloudPath(.puffy) }
        return sprite(pixels: 44, seed: 23,
                      shade: { $0.fill(shape(), 150) }, pink: { $0.fill(shape(), 215) },
                      ink: { ctx in ctx.pen(3.4); ctx.addPath(shape()); ctx.strokePath() })
    }()

    /// The update's sky: one blue family in a few shades, each cloud its own.
    enum Blue {
        static let mist: (UInt8, UInt8, UInt8) = (204, 222, 244)
        static let pale: (UInt8, UInt8, UInt8) = (176, 204, 236)
        static let sky: (UInt8, UInt8, UInt8) = (150, 186, 232)
        static let mid = Riso.sky
        static let dusk: (UInt8, UInt8, UInt8) = (98, 138, 204)
    }

    enum CloudShape { case puffy, long, round, wisp }

    nonisolated private static func cloudPath(_ kind: CloudShape) -> CGPath {
        func union(_ parts: [CGPath]) -> CGPath { parts.dropFirst().reduce(parts[0]) { $0.union($1) } }
        func base(_ x0: CGFloat, _ x1: CGFloat, _ y: CGFloat, _ h: CGFloat) -> CGPath {
            CGPath(roundedRect: CGRect(x: x0, y: y, width: x1 - x0, height: h), cornerWidth: h / 2, cornerHeight: h / 2, transform: nil)
        }
        switch kind {
        case .puffy: return union([ellipse(24, 46, 28, 26), ellipse(42, 36, 36, 34), ellipse(58, 46, 26, 24), base(12, 68, 44, 14)])
        case .long: return union([ellipse(14, 50, 18, 16), ellipse(30, 44, 24, 24), ellipse(48, 41, 26, 26), ellipse(66, 48, 20, 18),
                                  base(5, 76, 47, 11)])
        case .round: return union([ellipse(34, 42, 40, 38), ellipse(56, 50, 24, 20), base(16, 66, 48, 12)])
        case .wisp: return union([ellipse(40, 50, 70, 14), ellipse(30, 45, 26, 14), ellipse(52, 44, 22, 13)])
        }
    }

    /// A sky cloud: its blue printed a pixel up-left of the pen edge, like the pink plate
    /// elsewhere. `edge` swaps the espresso pen for a deeper blue (the faint wisp).
    static func skyCloud(_ kind: CloudShape, _ fill: (UInt8, UInt8, UInt8), density: CGFloat = 80,
                         edge: (UInt8, UInt8, UInt8)? = nil, seed: Int) -> NSImage {
        let s: CGFloat = 44.0 / 80
        let path = cloudPath(kind)
        return RisoArt.print(pixels: 44, seed: seed, plates: [
            (fill, -1, -1, { ctx in ctx.scaleBy(x: s, y: s); ctx.addPath(path); ctx.setFillColor(gray: density / 255, alpha: 1); ctx.fillPath() }),
            (edge ?? Riso.espresso, 0, 0, { ctx in ctx.scaleBy(x: s, y: s); ctx.pen(3.4); ctx.addPath(path); ctx.strokePath() }),
        ])
    }

    /// A sun with short rays, printed sunflower yellow so it never reads as another bloom.
    static let sun = sprite(pixels: 44, seed: 25, colour: Riso.sunflower,
        shade: { $0.fill(ellipse(41, 41, 34, 34), 160) },
        pink: { $0.fill(ellipse(40, 40, 32, 32), 45) },
        ink: { ctx in
            ctx.pen(3.4)
            ctx.addPath(ellipse(40, 40, 32, 32)); ctx.strokePath()
            for k in 0..<8 {
                let a = Double(k) / 8 * 2 * .pi
                ctx.move(to: CGPoint(x: 40 + 22 * cos(a), y: 40 + 22 * sin(a)))
                ctx.addLine(to: CGPoint(x: 40 + 32 * cos(a), y: 40 + 32 * sin(a)))
                ctx.strokePath()
            }
        })

    // MARK: Helpers

    nonisolated private static func ellipse(_ cx: CGFloat, _ cy: CGFloat, _ w: CGFloat, _ h: CGFloat) -> CGPath {
        CGPath(ellipseIn: CGRect(x: cx - w / 2, y: cy - h / 2, width: w, height: h), transform: nil)
    }

    /// Three plates drawn on an 80×80 canvas and printed at `pixels`.
    /// `colour` is the middle plate's ink: pink unless the sprite has its own (sun, butterflies).
    private static func sprite(pixels: Int, seed: Int, colour: (UInt8, UInt8, UInt8) = Riso.pink,
                               shade: @escaping (CGContext) -> Void,
                               pink: @escaping (CGContext) -> Void, ink: @escaping (CGContext) -> Void) -> NSImage {
        let s = CGFloat(pixels) / 80
        func scaled(_ draw: @escaping (CGContext) -> Void) -> (CGContext) -> Void { { ctx in ctx.scaleBy(x: s, y: s); draw(ctx) } }
        return RisoArt.print(pixels: pixels, seed: seed, plates: [
            (Riso.crema, 1, 1, scaled(shade)),
            (colour, -1, -1, scaled(pink)),
            (Riso.espresso, 0, 0, scaled(ink)),
        ])
    }
}

private extension CGContext {
    func pen(_ width: CGFloat) {
        setStrokeColor(gray: 25 / 255, alpha: 1)
        setLineWidth(width)
        setLineCap(.round)
        setLineJoin(.round)
    }
    func fill(_ path: CGPath, _ gray: CGFloat) { addPath(path); setFillColor(gray: gray / 255, alpha: 1); fillPath() }
}
