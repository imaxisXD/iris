import AppKit
import SwiftUI

// The coffee zine's moves, in Iris's materials (see DESIGN.md, "Where the look comes from"):
// ink labels, running heads, hand lettering over ghost words, the brush X, printed plates,
// footnotes, stacked figures and text up an edge.

/// A name or one fact printed as a solid ink strip with paper-coloured type, like a label
/// gun or the zine's "Espresso". Square, no outline. For the name of a thing (a tone, an app)
/// or the one fact a screen wants remembered, never for a status word (those stay brush).
struct InkLabel: View {
    let text: String
    var size: CGFloat = 11

    var body: some View {
        Text(text)
            .font(DS.Font.martian(size, weight: 430, width: 87.5))
            .foregroundStyle(DS.Surface.overlay)
            .lineLimit(1)
            .truncationMode(.tail)
            .padding(.horizontal, size * 0.62)
            .padding(.vertical, size * 0.26)
            .background(Rectangle().printedInk(Riso.ink, seed: text.count + 3))
    }
}

/// A name on a strip of masking tape, with the app's icon stuck on its corner at a tilt like
/// a sticker: the app Replace types into, beside the button. Pale, so it never reads as a
/// second black button the way an `InkLabel` there would. A long name is cut short.
struct TapeLabel: View {
    let text: String
    /// Bundle ID of the app whose real icon goes on the corner; nil for none.
    var appIcon: String? = nil
    var size: CGFloat = 10.5

    var body: some View {
        Text(text)
            .font(DS.Font.martian(size, weight: 430, width: 87.5))
            .foregroundStyle(DS.Ink.i12)
            .lineLimit(1)
            .truncationMode(.tail)
            .padding(.horizontal, size * 0.62 + 4)
            .padding(.vertical, size * 0.26)
            .background(Tape().fill(DS.Hue.tape).blendMode(.multiply))
            .rotationEffect(.degrees(-1.5))
            .padding(.leading, appIcon == nil ? 0 : 12)
            .overlay(alignment: .leading) {
                if let appIcon {
                    AppIconView(bundleID: appIcon, size: 22)
                        .rotationEffect(.degrees(-9))
                        .offset(x: -4, y: -5)
                }
            }
            .padding(.leading, appIcon == nil ? 0 : 4)
    }

    /// A strip of tape: torn, zigzag ends and edges a touch uneven.
    struct Tape: Shape {
        func path(in r: CGRect) -> Path {
            var rng = SeededRandom(seed: 12)
            var p = Path()
            let teeth = 5
            p.move(to: CGPoint(x: r.minX + 2, y: r.minY + 0.5))
            p.addLine(to: CGPoint(x: r.maxX - 2, y: r.minY))
            for k in 1...teeth {
                let y = r.minY + r.height * CGFloat(k) / CGFloat(teeth)
                p.addLine(to: CGPoint(x: r.maxX - (k.isMultiple(of: 2) ? 0 : 2.5) - rng.next(), y: y))
            }
            p.addLine(to: CGPoint(x: r.minX + 1.5, y: r.maxY - 0.5))
            for k in (0..<teeth).reversed() {
                let y = r.minY + r.height * CGFloat(k) / CGFloat(teeth)
                p.addLine(to: CGPoint(x: r.minX + (k.isMultiple(of: 2) ? 0 : 2.5) + rng.next(), y: y))
            }
            p.closeSubpath()
            return p
        }
    }
}

/// The strip of small columns across the top of a page: where you are, then a plain fact
/// or two, each over a short pen rule.
struct RunningHead: View {
    let items: [String]

    var body: some View {
        HStack(alignment: .top, spacing: 18) {
            ForEach(Array(items.enumerated()), id: \.offset) { i, item in
                VStack(alignment: .leading, spacing: 5) {
                    Text(item).font(DS.Font.micro).foregroundStyle(DS.Ink.i10)
                        .fixedSize(horizontal: false, vertical: true)
                    PenRule(seed: 40 + i * 7, ink: DS.Ink.i12.opacity(0.35)).frame(width: 64)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// A ghost word: chunky caps broken over two lines (CRU / SH), tight, in a pale ink with the
/// print's grain through it. Never text you read.
struct GhostWord: View {
    /// The pale riso inks a ghost can print in. Each Settings page takes one, so the pages
    /// don't all look alike.
    enum Ink {
        case khaki, pink, sky, tan
        var color: Color {
            let c: (Double, Double, Double) = switch self {
            case .khaki: (0.89, 0.87, 0.80)
            case .pink: (0.97, 0.83, 0.90)
            case .sky: (0.82, 0.88, 0.96)
            case .tan: (0.93, 0.85, 0.77)
            }
            return Color(red: c.0, green: c.1, blue: c.2)
        }
    }

    let word: String
    var size: CGFloat = 120
    var ink: Ink = .khaki

    private var lines: [String] {
        let caps = Array(word.uppercased())
        let cut = (caps.count + 1) / 2
        return caps.count < 4 ? [String(caps)] : [String(caps[..<cut]), String(caps[cut...])]
    }

    private var letters: some View {
        VStack(alignment: .leading, spacing: -size * 0.3) {
            ForEach(lines, id: \.self) { line in
                Text(line).font(DS.Font.chunky(size)).fixedSize()
            }
        }
    }

    var body: some View {
        // A pale ink, salted with paper-coloured grain like the zine's CRUSH close up.
        letters.hidden().overlay {
            ink.color
                .overlay { Image(nsImage: Grain.salt).resizable(resizingMode: .tile).opacity(0.9) }
                .mask { letters }
        }
        .accessibilityHidden(true)
    }
}

extension Grain {
    /// Fine paper-coloured specks, a pixel each, for salting a ghost word.
    static let salt: NSImage = {
        let size = 128
        var rng = SeededRandom(seed: 17)
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
                                   samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                                   bytesPerRow: size * 4, bitsPerPixel: 32)!
        let data = rep.bitmapData!
        for i in 0..<(size * size) {
            // Premultiplied paper colour; about a third of the pixels show some paper.
            let alpha = max(0, min(1, (rng.next() - 0.62) * 2.2))
            data[i * 4] = UInt8(252 * alpha); data[i * 4 + 1] = UInt8(249 * alpha)
            data[i * 4 + 2] = UInt8(246 * alpha); data[i * 4 + 3] = UInt8(255 * alpha)
        }
        let image = NSImage(size: NSSize(width: size / 2, height: size / 2))
        image.addRepresentation(rep)
        return image
    }()
}

/// Hand lettering the way the zine draws GRiND or CappUccino: every letter its own. A fat brush
/// cap to start, then letters sketched in a few loose strokes, fat again, hollow with pink
/// printed off register inside, scribbled round the outline, and a small lowercase i. One word
/// or a short label; decoration, VoiceOver skips it.
struct HandWord: View {
    let word: String
    var size: CGFloat = 60
    var seed: Int = 4
    private let ink = DS.Ink.i12
    /// Or say exactly how each letter is drawn, in order (a wordmark that must look the same
    /// every time). `.small` draws a lowercase i.
    var pattern: [HandStyle]? = nil
    /// How heavy the pen is round hollow letters (1 is the usual line).
    var weight: CGFloat = 1

    var body: some View {
        let layout = HandLayout.make(word, size: size, seed: seed, pattern: pattern)
        // Fat letters and the pink inside hollow ones are areas of ink: printed, like the plates.
        let inkScreen = Image(nsImage: RisoArt.screen(points: CGSize(width: 96, height: 96), rgb: Riso.ink, seed: seed + 3, grid: 0.22, specks: 0.35))
        let pinkScreen = Image(nsImage: RisoArt.screen(points: CGSize(width: 96, height: 96), rgb: Riso.pink, seed: seed + 5, grid: 0.55, specks: 0.6))
        Canvas { ctx, _ in
            for letter in layout.letters {
                var c = ctx
                c.translateBy(x: letter.center.x, y: letter.center.y)
                c.rotate(by: .degrees(letter.tilt))
                c.translateBy(x: -letter.center.x, y: -letter.center.y)
                draw(letter, in: c, inkScreen: inkScreen, pinkScreen: pinkScreen)
            }
        }
        .frame(width: layout.size.width, height: layout.size.height)
        .accessibilityHidden(true)
    }

    private func draw(_ letter: HandLayout.Letter, in c: GraphicsContext, inkScreen: Image, pinkScreen: Image) {
        /// The context nudged and turned about the letter's middle, for one more pass of the pen.
        func pass(_ n: Int, spread: CGFloat, turn: Double) -> GraphicsContext {
            var p = c
            let d = size * spread * CGFloat(n)
            p.translateBy(x: letter.center.x, y: letter.center.y)
            p.rotate(by: .degrees(Double(n) * turn - turn))
            p.scaleBy(x: 1 + CGFloat(n) * 0.02, y: 1 - CGFloat(n) * 0.015)
            p.translateBy(x: -letter.center.x + (n == 1 ? d : -d * 0.5), y: -letter.center.y + (n == 2 ? d * 0.6 : -d * 0.3))
            return p
        }
        switch letter.style {
        case .fat, .small:
            c.fill(letter.path, with: .tiledImage(inkScreen))
        case .sketch:
            // A thin marker letter's outline traced twice in a fine pen: every stroke of the
            // letter shows as three or four loose lines, like the zine's R in GRiND.
            for n in 0..<2 {
                pass(n, spread: 0.03, turn: 3).stroke(letter.path, with: .color(ink),
                    style: StrokeStyle(lineWidth: size * (n == 0 ? 0.026 : 0.02), lineCap: .round, lineJoin: .round))
            }
        case .scribble:
            for n in 0..<3 {
                pass(n, spread: 0.026, turn: 2.6).stroke(letter.path, with: .color(ink),
                    style: StrokeStyle(lineWidth: size * [0.042, 0.03, 0.024][n], lineCap: .round, lineJoin: .round))
            }
        case .hollow:
            // The pink plate printed a little off the ink, like the zine's cups and brewers.
            var pink = c
            pink.translateBy(x: size * 0.05, y: size * 0.04)
            pink.fill(letter.path, with: .tiledImage(pinkScreen))
            c.stroke(letter.path, with: .color(ink), style: StrokeStyle(lineWidth: size * 0.05 * weight, lineJoin: .round))
        }
    }
}

/// The ways a `HandWord` letter can be drawn.
enum HandStyle { case fat, sketch, hollow, scribble, small }

/// Where each letter of a `HandWord` goes and how it's drawn.
private struct HandLayout {
    typealias Style = HandStyle
    struct Letter { var path: Path; var center: CGPoint; var tilt: Double; var style: Style }
    var letters: [Letter] = []
    var size: CGSize = .zero

    /// After the first fat letter, the rest take turns, so no two neighbours are drawn alike.
    static let turns: [Style] = [.sketch, .fat, .hollow, .scribble, .fat, .sketch, .fat]

    static func make(_ word: String, size: CGFloat, seed: Int, pattern: [HandStyle]? = nil) -> HandLayout {
        let brush = CTFontCreateWithName(DS.Font.Name.fat as CFString, size, nil)
        // Mansalva's caps are shorter than Knewave's; scale it so the sketched letters stand as tall.
        let thin = CTFontCreateWithName(DS.Font.Name.sketch as CFString, size * 1.22, nil)
        var rng = SeededRandom(seed: seed + word.count)
        var out = HandLayout()
        let ascent = CTFontGetAscent(brush), descent = CTFontGetDescent(brush)
        let pad = size * 0.1  // room for the tilt, the scribble's overshoot and the pink's offset
        let baseline = pad + ascent * 1.08
        // Knewave leans right; take the lean out so the letters stand like the zine's.
        let upright = CGAffineTransform(a: 1, b: 0, c: -0.2, d: 1, tx: 0, ty: 0)
        var x = pad
        var turn = seed % turns.count
        for (i, ch) in word.enumerated() {
            if ch == " " { x += size * 0.32; continue }
            let style: Style
            if let pattern, !pattern.isEmpty { style = pattern[i % pattern.count] }
            else if i == 0 { style = .fat }
            else if ch.lowercased() == "i" { style = .small }
            else { style = turns[turn % turns.count]; turn += 1 }
            let font = style == .sketch ? thin : brush
            let shown = style == .small ? "i" : ch.uppercased()
            var units = Array(shown.utf16)
            var glyph = CGGlyph()
            var stand = style == .sketch ? .identity : upright
            guard CTFontGetGlyphsForCharacters(font, &units, &glyph, 1),
                  let cg = CTFontCreatePathForGlyph(font, glyph, &stand) else { continue }
            var advance = CGSize.zero
            CTFontGetAdvancesForGlyphs(font, .horizontal, &glyph, &advance, 1)
            // The first letter is bigger, like the zine's G and C.
            let grow: CGFloat = i == 0 && pattern == nil ? 1.14 : 1
            let drop = (rng.next() - 0.5) * size * 0.08
            let place = CGAffineTransform(a: grow, b: 0, c: 0, d: -grow, tx: x, ty: baseline + drop)
            let path = Path(cg).applying(place)
            let box = path.boundingRect
            out.letters.append(Letter(path: path, center: CGPoint(x: box.midX, y: box.midY),
                                      tilt: Double((rng.next() - 0.5) * 8), style: style))
            x += advance.width * grow - size * (style == .sketch ? 0.0 : 0.03)
        }
        out.size = CGSize(width: x + pad + size * 0.08, height: baseline + descent * 0.6 + pad)
        return out
    }
}

/// A short line of small caps running up an edge (a rule, a plate), printed in the sky ink
/// through the riso screen, like a second drum's pass, on a strip of paper so a rule under it
/// breaks around the words: the zine's text up the side of a photo. A "✻" in the text is
/// setup's star, the headlines' ✻, drawn heavy between the words. A little heavy, so the light
/// ink holds at this size. Decoration.
struct EdgeText: View {
    let text: String

    var body: some View {
        let words = line.font(DS.Font.martian(8, weight: 600, width: 100)).tracking(2.4).fixedSize()
        // The words cut out of a printed sky plate: ink through the screen, never a flat colour.
        words.hidden()
            .overlay { Rectangle().printedInk(Riso.sky, seed: text.count * 3).mask { words } }
            .padding(.horizontal, 6)
            // On a strip of paper with the window's grain, so a rule under it breaks around the
            // words and the strip itself doesn't show.
            .background(DS.Surface.overlay.grain(0.22))
            .rotationEffect(.degrees(-90))
            .frame(width: 12, height: 12)
            .accessibilityHidden(true)
    }

    /// The words in caps, each ✻ set as the heavy star (Martian has none).
    private var line: Text {
        let parts = text.uppercased().components(separatedBy: "✻")
        return parts.dropFirst().reduce(Text(parts[0])) { line, part in
            line + Text("✻").font(.system(size: 9, weight: .black)).baselineOffset(-0.5) + Text(part)
        }
    }
}

/// A figure set big with its unit stacked small beside it, like the zine's "18G / IN":
/// "5.2" then "GB" over "on disk".
struct StackedFigure: View {
    let value: String
    let unit: String
    let note: String

    var body: some View {
        HStack(alignment: .center, spacing: 5) {
            Text(value).font(DS.Font.display(26)).tracking(-0.6).foregroundStyle(DS.Ink.i12)
            VStack(alignment: .leading, spacing: -1) {
                Text(unit.uppercased())
                Text(note.uppercased())
            }
            .font(DS.Font.martian(8.5, weight: 500, width: 87.5)).foregroundStyle(DS.Ink.i11)
        }
        .fixedSize()
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(value) \(unit) \(note)")
    }
}

/// A hand-lettered label with a pen arrow curling toward the thing it names, like the zine's
/// "Light", "Medium", "Dark" beside the beans. The pen arrow (`HandArrow`) leaves the label's
/// trailing side: `.left` curves up and points across, `.down` curls over and points down.
/// Flip it for a target on the left. Decoration: what it points at says the same in words.
struct HandNote: View {
    let text: String
    var size: CGFloat = 22
    var curl: HandArrow.Curl = .left
    var reach = CGSize(width: 34, height: 18)
    var flipped = false
    var seed = 3

    var body: some View {
        HStack(alignment: .center, spacing: 3) {
            if flipped { arrow }
            HandWord(word: text, size: size, seed: seed).rotationEffect(.degrees(-4))
            if !flipped { arrow }
        }
        .accessibilityHidden(true)
    }

    private var arrow: some View {
        HandArrow(curl: curl)
            .stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 1.6, lineCap: .round, lineJoin: .round))
            .frame(width: reach.width, height: reach.height)
            .scaleEffect(x: flipped ? -1 : 1)
            .offset(y: curl == .down ? reach.height / 2 : 0)
    }
}

/// A big drawing of one of Iris's icons, printed like the zine's brewers: a heavy ink line
/// with the pink plate landed a few dots off it, so the colour spills past the line.
struct RisoDrawing: View {
    let icon: RisoIcon
    var size: CGFloat = 84

    var body: some View {
        Image(nsImage: icon.poster(pixels: Int(size))).interpolation(.none).resizable()
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

/// The foot of a page printed solid in one ink, like the zine's half-page of pink: a stepped
/// sign-off in small paper-coloured mono at the left, the page's folio big at the right.
struct PageFoot: View {
    let folio: String
    let signOff: [String]
    var ink: ScreenPlate.Ink = .pink

    var body: some View {
        HStack(alignment: .bottom) {
            VStack(alignment: .leading, spacing: 1) {
                ForEach(Array(signOff.enumerated()), id: \.offset) { i, line in
                    Text(line).padding(.leading, CGFloat(i) * 18)
                }
            }
            // Heavier than body text, so it holds up against the plate's specks.
            .font(DS.Font.martian(11.5, weight: 600)).foregroundStyle(Color(red: 1, green: 0.985, blue: 0.97))
            Spacer()
            Text(folio).font(DS.Font.display(58)).tracking(-1.5)
                .foregroundStyle(Color(red: 1, green: 0.985, blue: 0.97))
        }
        .padding(.horizontal, 32).padding(.top, 26).padding(.bottom, 18)
        .frame(maxWidth: .infinity)
        .background(ScreenPlate(ink: ink, seed: folio.count * 5 + 3))
        .accessibilityHidden(true)
    }
}

/// The sky band a status is said on, once: the printed plate with the status as a ghost word
/// in paper along its bottom edge, cut by hand. The menu bar slip's header and Settings ›
/// Model & memory stand their garden on it. The ghost is an overlay, so a long word
/// ("DOWNLOADING") never widens the band.
struct StatusBand: View {
    let word: String
    let height: CGFloat
    /// The ghost word's size, and how far it hangs below the band's bottom edge.
    var size: CGFloat = 72
    var drop: CGFloat = 20
    /// Snapshots: draw this moment of the ticker.
    var frozenAt: Double? = nil

    var body: some View {
        ScreenPlate(ink: .sky, seed: 13)
            .frame(height: height)
            .overlay(alignment: .bottomLeading) {
                GhostTicker(text: word.uppercased(), size: size, frozenAt: frozenAt).offset(y: drop)
            }
            .clipped()
            .clipShape(HandRect(seed: 21, wobble: 0.5, closed: true))
    }
}

/// A ghost word in paper on a printed band. A word that fits stands still; one too long for
/// the band ("DOWNLOADING") runs past slowly like a ticker, round and round, a ✻ between
/// repeats, so the whole word is seen. Reduce Motion holds it still. Decoration: the status is
/// read out elsewhere.
struct GhostTicker: View {
    let text: String
    var size: CGFloat = 72
    /// Points a second, left to right.
    private let speed: CGFloat = 26
    /// Snapshots: draw this moment.
    var frozenAt: Double? = nil
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var ink: Color { DS.Surface.overlay.opacity(0.32) }

    var body: some View {
        GeometryReader { geo in
            let word = Self.width(of: text, size: size)
            if word <= geo.size.width - 8 || reduceMotion {
                Text(text).font(DS.Font.chunky(size)).foregroundStyle(ink).fixedSize()
                    .offset(x: -4)
                    .frame(maxHeight: .infinity, alignment: .bottomLeading)
            } else {
                let gap = size * 0.55
                let cycle = word + gap
                TimelineView(.animation(paused: frozenAt != nil)) { timeline in
                    let t = frozenAt ?? timeline.date.timeIntervalSinceReferenceDate
                    let travelled = CGFloat(t).truncatingRemainder(dividingBy: cycle / speed) * speed
                    let x = travelled - cycle
                    HStack(spacing: 0) {
                        ForEach(0..<3, id: \.self) { _ in
                            Text(text).font(DS.Font.chunky(size)).fixedSize()
                            Text("✻").font(.system(size: size * 0.42, weight: .black)).frame(width: gap)
                        }
                    }
                    .foregroundStyle(ink)
                    .fixedSize()
                    .offset(x: x)
                    .frame(maxHeight: .infinity, alignment: .bottomLeading)
                }
            }
        }
        .accessibilityHidden(true)
    }

    static func width(of text: String, size: CGFloat) -> CGFloat {
        guard let font = NSFont(name: DS.Font.Name.chunky, size: size) else { return 0 }
        return (text as NSString).size(withAttributes: [.font: font]).width
    }
}

/// Marks the recommended model: a pink starburst sticker with a pen tick, slapped on at an
/// angle, and its words beside it ("recommended" in brush, "for this Mac" under), so the
/// picture never has to be decoded. Decoration: the model's button says it aloud.
struct RecommendedMark: View {
    var body: some View {
        HStack(alignment: .center, spacing: 6) {
            ZStack {
                Burst(points: 14, inner: 0.78, seed: 3).printedInk(Riso.pink, seed: 9).offset(x: 2, y: 2)
                Burst(points: 14, inner: 0.78, seed: 3)
                    .stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 1.6, lineJoin: .round))
                HandTick().stroke(DS.Ink.i12, style: StrokeStyle(lineWidth: 3, lineCap: .round, lineJoin: .round))
                    .frame(width: 22, height: 18)
            }
            .frame(width: 52, height: 52)
            .rotationEffect(.degrees(-12))
            VStack(alignment: .leading, spacing: -3) {
                Text("recommended").font(DS.Font.brush(19)).foregroundStyle(DS.Hue.pinkText)
                Text("for this Mac").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
            }
            .fixedSize()
            .rotationEffect(.degrees(-3))
        }
        .accessibilityHidden(true)
    }

    /// A burst with `points` tips, its inner radius `inner` of the outer, each tip a touch
    /// uneven so it reads as cut by hand.
    struct Burst: Shape {
        let points: Int
        let inner: CGFloat
        var seed = 1
        func path(in r: CGRect) -> Path {
            var rng = SeededRandom(seed: seed)
            let c = CGPoint(x: r.midX, y: r.midY), radius = min(r.width, r.height) / 2
            var p = Path()
            for k in 0..<(points * 2) {
                let a = CGFloat(k) / CGFloat(points * 2) * 2 * .pi - .pi / 2
                let rad = (k.isMultiple(of: 2) ? radius : radius * inner) * (0.94 + rng.next() * 0.08)
                let pt = CGPoint(x: c.x + cos(a) * rad, y: c.y + sin(a) * rad)
                k == 0 ? p.move(to: pt) : p.addLine(to: pt)
            }
            p.closeSubpath()
            return p
        }
    }
}

/// One stroke of a loaded dry brush, like the zine's X over the cup: flat and fat, a touch
/// heavier where the brush lands, rough edges, square ends frayed into bristle tips.
struct DryBrushStroke: Shape {
    var seed: Int
    var from: UnitPoint
    var to: UnitPoint
    /// Width as a share of the stroke's length.
    var width: CGFloat = 0.1

    func path(in rect: CGRect) -> Path {
        var rng = SeededRandom(seed: seed)
        let a = CGPoint(x: rect.minX + from.x * rect.width, y: rect.minY + from.y * rect.height)
        let b = CGPoint(x: rect.minX + to.x * rect.width, y: rect.minY + to.y * rect.height)
        let dx = b.x - a.x, dy = b.y - a.y
        let length = max(hypot(dx, dy), 1)
        let dir = CGPoint(x: dx / length, y: dy / length)
        let n = CGPoint(x: -dir.y, y: dir.x)
        let w = length * width
        let bend = (rng.next() - 0.5) * length * 0.07
        let steps = 22
        var left: [CGPoint] = [], right: [CGPoint] = []
        for i in 0...steps {
            let t = CGFloat(i) / CGFloat(steps)
            let off = sin(t * .pi) * bend
            let c = CGPoint(x: a.x + dx * t + n.x * off, y: a.y + dy * t + n.y * off)
            let half = w / 2 * (1.12 - 0.22 * t)
            let l = half * (1 + (rng.next() - 0.5) * 0.16), r = half * (1 + (rng.next() - 0.5) * 0.16)
            left.append(CGPoint(x: c.x + n.x * l, y: c.y + n.y * l))
            right.append(CGPoint(x: c.x - n.x * r, y: c.y - n.y * r))
        }
        // Frayed ends: bristle tips of uneven length across the width.
        func fray(_ from: CGPoint, _ to: CGPoint, outward: CGPoint, reach: CGFloat) -> [CGPoint] {
            var pts: [CGPoint] = []
            let tips = 9
            for k in 0...(tips * 2) {
                let u = CGFloat(k) / CGFloat(tips * 2)
                let base = CGPoint(x: from.x + (to.x - from.x) * u, y: from.y + (to.y - from.y) * u)
                let out = k % 2 == 1 ? reach * (0.35 + rng.next() * 0.65) : reach * rng.next() * 0.15
                pts.append(CGPoint(x: base.x + outward.x * out, y: base.y + outward.y * out))
            }
            return pts
        }
        var p = Path()
        p.move(to: left[0])
        for pt in left.dropFirst() { p.addLine(to: pt) }
        for pt in fray(left[steps], right[steps], outward: dir, reach: w * 0.32) { p.addLine(to: pt) }
        for pt in right.reversed().dropFirst() { p.addLine(to: pt) }
        for pt in fray(right[0], left[0], outward: CGPoint(x: -dir.x, y: -dir.y), reach: w * 0.22) { p.addLine(to: pt) }
        p.closeSubpath()
        return p
    }
}

/// The zine's X: two dry-brush strokes, the second laid over the first, printed through a
/// screen like the plates (`RisoArt.screen`: a faint grid, paper specks), with bristle streaks
/// dragged along each stroke where the brush ran dry.
struct DryBrushX: View {
    /// The red pen (`DS.Hue.red`) by default: this is the mark for what Delete would destroy.
    var rgb: RGB8 = Riso.red
    var seed = 3
    var width: CGFloat = 0.1
    /// 0 is flat ink; 1 is a well-worn dry brush.
    var dryness: CGFloat = 0.6

    private static let strokes: [(UnitPoint, UnitPoint)] = [
        (UnitPoint(x: 0.03, y: 0.08), UnitPoint(x: 0.97, y: 0.95)),
        (UnitPoint(x: 0.96, y: 0.03), UnitPoint(x: 0.05, y: 0.93)),
    ]

    var body: some View {
        GeometryReader { geo in
            ZStack {
                Image(nsImage: RisoArt.screen(points: geo.size, rgb: rgb, seed: seed + 40))
                    .interpolation(.none).resizable()
                    .mask {
                        ZStack {
                            ForEach(0..<2, id: \.self) { k in
                                DryBrushStroke(seed: seed + k * 7, from: Self.strokes[k].0, to: Self.strokes[k].1, width: width)
                            }
                        }
                    }
                if dryness > 0 {
                    bristles(in: geo.size).blendMode(.destinationOut)
                }
            }
            .compositingGroup()
        }
        .accessibilityHidden(true)
    }

    /// Thin gaps dragged along each stroke: many faint, a few that open to paper, longest
    /// near where the brush lifts. Plus loose paper specks where the ink ran thin.
    private func bristles(in size: CGSize) -> some View {
        Canvas { ctx, _ in
            var rng = SeededRandom(seed: seed * 31 + 5)
            for k in 0..<2 {
                let (f, t) = Self.strokes[k]
                let a = CGPoint(x: f.x * size.width, y: f.y * size.height)
                let b = CGPoint(x: t.x * size.width, y: t.y * size.height)
                let len = hypot(b.x - a.x, b.y - a.y)
                let d = CGPoint(x: (b.x - a.x) / len, y: (b.y - a.y) / len), n = CGPoint(x: -d.y, y: d.x)
                let w = len * width
                let count = Int(6 + dryness * 10)
                for _ in 0..<count {
                    let across = (rng.next() - 0.5) * w * 0.9
                    // Mostly toward the lifting end, some anywhere.
                    let start = rng.next() < 0.7 ? 0.45 + rng.next() * 0.4 : rng.next() * 0.7
                    let end = min(1.03, start + 0.08 + rng.next() * (0.15 + dryness * 0.25))
                    var p = Path()
                    p.move(to: CGPoint(x: a.x + d.x * len * start + n.x * across, y: a.y + d.y * len * start + n.y * across))
                    p.addLine(to: CGPoint(x: a.x + d.x * len * end + n.x * across, y: a.y + d.y * len * end + n.y * across))
                    let open = rng.next() < 0.3
                    ctx.stroke(p, with: .color(.black.opacity(open ? 0.9 : 0.25 + rng.next() * 0.3)),
                               style: StrokeStyle(lineWidth: max(0.4, w * (open ? 0.05 : 0.035 + rng.next() * 0.03)), lineCap: .round))
                }
                // Specks: tiny pits in the ink.
                for _ in 0..<Int(10 + dryness * 40) {
                    let along = rng.next(), across = (rng.next() - 0.5) * w * 0.85
                    let c = CGPoint(x: a.x + d.x * len * along + n.x * across, y: a.y + d.y * len * along + n.y * across)
                    let r = max(0.35, w * (0.02 + rng.next() * 0.03))
                    ctx.fill(Path(ellipseIn: CGRect(x: c.x - r, y: c.y - r, width: r * 2, height: r * 2)),
                             with: .color(.black.opacity(0.5 + rng.next() * 0.5)))
                }
            }
        }
    }
}

/// A heavy brush X over something, to say "not here" (an app Iris is turned off in). In
/// ink, not the red pen: it's a choice, not a failure.
struct BrushCross: View {
    var seed: Int = 3

    var body: some View {
        // The zine's dry brush, in ink: crossing an app out is a choice, not a deletion.
        DryBrushX(rgb: Riso.ink, seed: seed, width: 0.12, dryness: 0.4)
    }
}


/// A plate printed through a coarse screen, like the zine's flat colour areas: mostly ink,
/// with the paper showing through in a dot pattern that thickens and thins across it, so it
/// never reads as a flat fill. Generated once per size and ink.
struct ScreenPlate: View {
    enum Ink: String, CaseIterable {
        /// Pink is for areas without the flower on them (a page's foot); setup's plate never.
        case tan, sky, pink
        var rgb: RGB8 {
            switch self {
            case .tan: Riso.tan
            case .sky: Riso.sky
            case .pink: Riso.platePink
            }
        }
    }

    var ink: Ink = .tan
    var seed: Int = 7

    var body: some View {
        GeometryReader { geo in
            Image(nsImage: RisoArt.screen(points: geo.size, rgb: ink.rgb, seed: seed))
                .interpolation(.none).resizable()
        }
        .accessibilityHidden(true)
    }
}

extension NSImage {
    /// The image drawn once into a plain bitmap, so an app icon (a stack of many sizes)
    /// renders the same in a window and in a snapshot.
    func flattened(points: CGFloat = 32) -> NSImage {
        let px = Int(points * 2)
        guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
                                         samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                                         bytesPerRow: 0, bitsPerPixel: 0) else { return self }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        draw(in: NSRect(x: 0, y: 0, width: px, height: px), from: .zero, operation: .sourceOver, fraction: 1)
        NSGraphicsContext.restoreGraphicsState()
        let image = NSImage(size: NSSize(width: points, height: points))
        image.addRepresentation(rep)
        return image
    }
}

extension RisoArt {
    @MainActor private static var screens: [String: NSImage] = [:]

    /// A screen-printed plate `points` big, a point to a dot, the way the zine's flat colour
    /// looks close up: nearly solid ink, a faint printing grid running through it, and sparse
    /// specks of paper that gather in a few places where the drum ran dry.
    /// `grid` and `specks` scale the printing grid and the paper specks: 1 for a big plate,
    /// less for a small surface with words on it, where they'd read as marks on the letters.
    @MainActor static func screen(points: CGSize, rgb: (UInt8, UInt8, UInt8), seed: Int,
                                  grid: CGFloat = 1, specks: CGFloat = 1) -> NSImage {
        let w = max(1, Int(points.width)), h = max(1, Int(points.height))
        let key = "\(w)x\(h)-\(rgb.0)-\(rgb.1)-\(rgb.2)-\(seed)-\(grid)-\(specks)"
        if let cached = screens[key] { return cached }
        var rng = SeededRandom(seed: seed)
        // Value noise on a lattice, smoothly interpolated: a broad one for the ink thinning
        // and thickening across the sheet, a fine one for worn patches where the paper shows.
        struct Noise {
            let cell: Int, gw: Int
            var lattice: [CGFloat]
            init(cell: Int, w: Int, h: Int, rng: inout SeededRandom) {
                self.cell = cell
                gw = w / cell + 2
                lattice = (0..<(gw * (h / cell + 2))).map { _ in rng.next() }
            }
            func at(_ x: Int, _ y: Int) -> CGFloat {
                func smooth(_ t: CGFloat) -> CGFloat { t * t * (3 - 2 * t) }
                let gx = x / cell, gy = y / cell
                let fx = smooth(CGFloat(x % cell) / CGFloat(cell)), fy = smooth(CGFloat(y % cell) / CGFloat(cell))
                let a = lattice[gy * gw + gx], b = lattice[gy * gw + gx + 1]
                let c = lattice[(gy + 1) * gw + gx], d = lattice[(gy + 1) * gw + gx + 1]
                let top = a + (b - a) * fx
                return top + ((c + (d - c) * fx) - top) * fy
            }
        }
        let dry = Noise(cell: 70, w: w, h: h, rng: &rng)  // where the specks gather
        let tone = Noise(cell: 46, w: w, h: h, rng: &rng)  // ink a touch heavier or lighter
        // Paper colour, for the specks and the grid's light lines.
        let paper = (CGFloat(Riso.paper.0), CGFloat(Riso.paper.1), CGFloat(Riso.paper.2))
        var out = [UInt8](repeating: 0, count: w * h * 4)
        for y in 0..<h {
            for x in 0..<w {
                let o = (y * w + x) * 4
                out[o + 3] = 255
                // A speck: rare everywhere, up to one in twelve where the drum ran dry; some are two dots wide.
                let chance = (0.004 + 0.085 * max(0, dry.at(x, y) - 0.58)) * specks
                if rng.next() < chance || (x > 0 && out[o - 4 + 3] == 254 && rng.next() < 0.35 * specks) {
                    // Soft specks on small surfaces: the ink thins rather than leaving bare paper.
                    let k: CGFloat = specks < 1 ? 0.45 : 1
                    out[o] = UInt8(CGFloat(rgb.0) + (paper.0 - CGFloat(rgb.0)) * k)
                    out[o + 1] = UInt8(CGFloat(rgb.1) + (paper.1 - CGFloat(rgb.1)) * k)
                    out[o + 2] = UInt8(CGFloat(rgb.2) + (paper.2 - CGFloat(rgb.2)) * k)
                    out[o + 3] = 254  // marks a speck so its neighbour can widen it
                    continue
                }
                // The printing grid: every third row and column a little lighter; the ink itself drifts ±5%.
                var lift = ((y % 3 == 0 ? 0.1 : 0) + (x % 3 == 0 ? 0.07 : 0)) * grid
                lift += (tone.at(x, y) - 0.5) * 0.1
                lift = max(0, min(0.3, lift))
                out[o] = UInt8(CGFloat(rgb.0) + (paper.0 - CGFloat(rgb.0)) * lift)
                out[o + 1] = UInt8(CGFloat(rgb.1) + (paper.1 - CGFloat(rgb.1)) * lift)
                out[o + 2] = UInt8(CGFloat(rgb.2) + (paper.2 - CGFloat(rgb.2)) * lift)
            }
        }
        let image = RisoArt.image(rgba: out, width: w, height: h)
        screens[key] = image
        return image
    }
}


// MARK: - Printed ink


/// Ink printed through the riso screen (`RisoArt.screen`): nearly solid, a faint grid, paper
/// specks where the drum ran dry, the way the zine's flat colour looks close up.
struct PrintedInk: View {
    let rgb: RGB8
    var seed = 7
    /// Dark ink shows the screen far more than a pale one, so it gets a lighter touch.
    private var dark: Bool { Int(rgb.0) + Int(rgb.1) + Int(rgb.2) < 240 }
    var body: some View {
        GeometryReader { geo in
            Image(nsImage: RisoArt.screen(points: geo.size, rgb: rgb, seed: seed,
                                          grid: dark ? 0.22 : 0.55, specks: dark ? 0.35 : 0.6))
                .interpolation(.none).resizable()
        }
        .accessibilityHidden(true)
    }
}

extension Shape {
    /// The shape filled with ink printed through the screen. Use this, never a flat `.fill`,
    /// for any area of ink people see: plates, strips, keys, markers (DESIGN.md, "Printed, never flat").
    @MainActor func printedInk(_ rgb: RGB8, seed: Int = 7) -> some View {
        PrintedInk(rgb: rgb, seed: seed).clipShape(self)
    }
}

/// The pink marker under a brush word ("looks good", the chosen tone, the chosen menu item):
/// a quick dry-brush swipe printed through the screen, frayed where the marker lifts.
struct MarkerSwipe: View {
    var height: CGFloat = 6
    var seed = 5

    var body: some View {
        GeometryReader { geo in
            PrintedInk(rgb: Riso.pink, seed: seed + 20)
                .mask(DryBrushStroke(seed: seed, from: UnitPoint(x: 0, y: 0.5), to: UnitPoint(x: 1, y: 0.5),
                                     width: geo.size.height / max(geo.size.width, 1) * 0.95))
        }
        .frame(height: height + 2)
        .blendMode(.multiply)
        .accessibilityHidden(true)
    }
}

// MARK: - Printed pen marks

/// A pen or brush stroke with body: a ribbon along a centre line, swelling where the pen
/// presses and thinning where it lifts, so it can be printed through the screen like ink.
struct InkRibbon: Shape {
    enum Line { case loop(seed: Int), under(seed: Int) }
    var line: Line
    var width: CGFloat
    /// Width at the start and end, as a share of `width`.
    var startTaper: CGFloat = 0.5
    var endTaper: CGFloat = 0.25
    /// Frayed bristle tips where the brush lifts.
    var fray = false
    /// How much of the stroke is drawn, for drawing it in (0...1).
    var drawn: CGFloat = 1

    var animatableData: CGFloat {
        get { drawn }
        set { drawn = newValue }
    }

    func centre(in rect: CGRect) -> [CGPoint] {
        switch line {
        case .loop(let seed):
            // The same loop as HandEllipse: a little over a full turn, a slight spiral. Kept
            // inside the frame (the spiral grows 10%), so the printed ink isn't cut off.
            let rect = rect.insetBy(dx: rect.width * 0.05 + width, dy: rect.height * 0.05 + width)
            var rng = SeededRandom(seed: seed)
            let start = -2.5 + rng.next() * 0.3, sweep = 2 * CGFloat.pi + 0.55, phase = rng.next() * 6.3
            var pts: [CGPoint] = []
            for i in 0...60 {
                let t = CGFloat(i) / 60
                let a: CGFloat = start + sweep * t
                let grow: CGFloat = 1 + 0.1 * t
                let o: CGFloat = sin(a * 3 + phase)
                let rx: CGFloat = rect.width / 2 * grow + o, ry: CGFloat = rect.height / 2 * grow + o
                pts.append(CGPoint(x: rect.midX + rx * cos(a), y: rect.midY + ry * sin(a)))
            }
            return pts
        case .under(let seed):
            var rng = SeededRandom(seed: seed)
            let bend = (rng.next() - 0.3) * rect.height * 0.8
            var pts: [CGPoint] = []
            for i in 0...24 {
                let t = CGFloat(i) / 24
                let x: CGFloat = rect.minX + rect.width * t
                let sag: CGFloat = sin(t * .pi) * bend
                let rise: CGFloat = t * rect.height * 0.25
                let jitter: CGFloat = (rng.next() - 0.5) * 0.3
                pts.append(CGPoint(x: x, y: rect.midY + sag - rise + jitter))
            }
            return pts
        }
    }

    func path(in rect: CGRect) -> Path {
        let all = centre(in: rect)
        let c = Array(all.prefix(max(0, Int((CGFloat(all.count) * min(1, max(0, drawn))).rounded()))))
        guard c.count > 2 else { return Path() }
        var rng = SeededRandom(seed: c.count * 7)
        var left: [CGPoint] = [], right: [CGPoint] = []
        for i in c.indices {
            let a = c[max(0, i - 1)], b = c[min(c.count - 1, i + 1)]
            let len = max(hypot(b.x - a.x, b.y - a.y), 0.001)
            let n = CGPoint(x: -(b.y - a.y) / len, y: (b.x - a.x) / len)
            let t = CGFloat(i) / CGFloat(all.count - 1)
            // Presses in fast, lifts slowly.
            let press = min(1, t / 0.12) * (1 - startTaper) + startTaper
            let lift = min(1, (1 - t) / 0.3) * (1 - endTaper) + endTaper
            let half = width / 2 * min(press, lift) * (1 + (rng.next() - 0.5) * 0.14)
            left.append(CGPoint(x: c[i].x + n.x * half, y: c[i].y + n.y * half))
            right.append(CGPoint(x: c[i].x - n.x * half, y: c[i].y - n.y * half))
        }
        var p = Path()
        p.move(to: left[0])
        for q in left.dropFirst() { p.addLine(to: q) }
        if fray && c.count == all.count {
            let e = c.count - 1, a = c[e - 1], b = c[e]
            let len = max(hypot(b.x - a.x, b.y - a.y), 0.001), d = CGPoint(x: (b.x - a.x) / len, y: (b.y - a.y) / len)
            for k in 0...8 {
                let u = CGFloat(k) / 8
                let base = CGPoint(x: left[e].x + (right[e].x - left[e].x) * u, y: left[e].y + (right[e].y - left[e].y) * u)
                let out = k % 2 == 1 ? width * (0.3 + rng.next() * 0.7) : 0
                p.addLine(to: CGPoint(x: base.x + d.x * out, y: base.y + d.y * out))
            }
        }
        for q in right.reversed() { p.addLine(to: q) }
        p.closeSubpath()
        return p
    }
}


/// A ring round something chosen or something wrong, drawn like the zine's brush: one loop a
/// little past full, heavy where the brush lands, frayed where it lifts, printed through the
/// screen. Pink for chosen, the red pen for a problem. Sized like the `HandEllipse` it replaces:
/// give it the frame of the thing it rings. Its weight follows its size.
struct BrushRing: View {
    var rgb: RGB8 = Riso.pink
    var seed = 13
    /// Draws it in, 0 to 1.
    var drawn: CGFloat = 1

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width, h = geo.size.height
            let weight = max(2.2, min(w, h) * 0.062)
            // The ribbon keeps its loop inside its frame, so give it a frame big enough that
            // the loop lands where a HandEllipse in this frame would.
            let bw = (w + 2 * weight) / 0.9, bh = (h + 2 * weight) / 0.9
            InkRibbon(line: .loop(seed: seed), width: weight, startTaper: 0.8, endTaper: 0.5, fray: true, drawn: drawn)
                .printedInk(rgb, seed: seed + 3)
                .frame(width: bw, height: bh)
                .position(x: w / 2, y: h / 2)
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// One pen stroke through points (in the unit square), smoothed, pressing in at the start and
/// lifting at the end like the link underline. A shape: print it with `printedInk`.
struct PenStroke: Shape {
    var points: [UnitPoint]
    var width: CGFloat
    /// Width at the start and end, as a share of `width`.
    var startTaper: CGFloat = 0.55
    var endTaper: CGFloat = 0.3
    var seed = 1

    func path(in rect: CGRect) -> Path {
        let pts = points.map { CGPoint(x: rect.minX + $0.x * rect.width, y: rect.minY + $0.y * rect.height) }
        guard pts.count > 1 else { return Path() }
        // Catmull-Rom through the points, sampled finely.
        var c: [CGPoint] = []
        for i in 0..<(pts.count - 1) {
            let a = pts[max(0, i - 1)], b = pts[i], d = pts[i + 1], e = pts[min(pts.count - 1, i + 2)]
            for k in 0..<10 {
                let t = CGFloat(k) / 10, t2 = t * t, t3 = t2 * t
                let x: CGFloat = 0.5 * ((2 * b.x) + (-a.x + d.x) * t + (2 * a.x - 5 * b.x + 4 * d.x - e.x) * t2 + (-a.x + 3 * b.x - 3 * d.x + e.x) * t3)
                let y: CGFloat = 0.5 * ((2 * b.y) + (-a.y + d.y) * t + (2 * a.y - 5 * b.y + 4 * d.y - e.y) * t2 + (-a.y + 3 * b.y - 3 * d.y + e.y) * t3)
                c.append(CGPoint(x: x, y: y))
            }
        }
        c.append(pts[pts.count - 1])
        var rng = SeededRandom(seed: seed)
        var left: [CGPoint] = [], right: [CGPoint] = []
        for i in c.indices {
            let a = c[max(0, i - 1)], b = c[min(c.count - 1, i + 1)]
            let len = max(hypot(b.x - a.x, b.y - a.y), 0.001)
            let n = CGPoint(x: -(b.y - a.y) / len, y: (b.x - a.x) / len)
            let t = CGFloat(i) / CGFloat(c.count - 1)
            let press: CGFloat = min(1, t / 0.15) * (1 - startTaper) + startTaper
            let lift: CGFloat = min(1, (1 - t) / 0.35) * (1 - endTaper) + endTaper
            let half: CGFloat = width / 2 * min(press, lift) * (1 + (rng.next() - 0.5) * 0.12)
            left.append(CGPoint(x: c[i].x + n.x * half, y: c[i].y + n.y * half))
            right.append(CGPoint(x: c[i].x - n.x * half, y: c[i].y - n.y * half))
        }
        var p = Path()
        p.move(to: left[0])
        for q in left.dropFirst() { p.addLine(to: q) }
        for q in right.reversed() { p.addLine(to: q) }
        p.closeSubpath()
        return p
    }
}

/// An arrow drawn with the pen and printed: the shaft, then the two barbs flicked out from
/// the tip. Every arrow on a button or link: Back, turning pages, "Read what changed", a link
/// that opens in the browser (pointing right, turned up). Never a stroked curl (`HandArrow`).
struct PenArrow: View {
    var rgb: RGB8 = Riso.ink
    var seed = 3
    var weight: CGFloat = 2.6
    /// Points left (back) unless this is set.
    var right = false

    /// Beside caption words (`TurnButton`, `PenLink`): one size, so every such arrow matches.
    static let besideWords = CGSize(width: 22, height: 12)
    static let besideWordsWeight: CGFloat = 2.3

    var body: some View {
        PrintedInk(rgb: rgb, seed: seed)
            .mask {
                ZStack {
                    PenStroke(points: [UnitPoint(x: 0.97, y: 0.56), UnitPoint(x: 0.6, y: 0.5), UnitPoint(x: 0.3, y: 0.52), UnitPoint(x: 0.05, y: 0.5)],
                              width: weight, startTaper: 0.7, endTaper: 0.8, seed: seed)
                    PenStroke(points: [UnitPoint(x: 0.04, y: 0.5), UnitPoint(x: 0.2, y: 0.28), UnitPoint(x: 0.36, y: 0.08)],
                              width: weight, startTaper: 0.9, endTaper: 0.35, seed: seed + 1)
                    PenStroke(points: [UnitPoint(x: 0.04, y: 0.5), UnitPoint(x: 0.21, y: 0.72), UnitPoint(x: 0.38, y: 0.93)],
                              width: weight, startTaper: 0.9, endTaper: 0.35, seed: seed + 2)
                }
                .scaleEffect(x: right ? -1 : 1, y: 1)
            }
            .accessibilityHidden(true)
    }
}

/// One loaded brush stroke, the primary button's face: ragged bristle ends (the brush lands
/// blunter on the left and lifts in long tips on the right), edges that wander, and dry streaks
/// and pits cut out of the ink, printed through the screen. Everything is measured from the
/// ends in points, so a longer stroke only grows in the middle: with the same seed, every
/// button is the same stroke of the same brush.
struct BrushSlab: View {
    let rgb: RGB8
    var seed: Int
    /// How dry the brush ran: more streaks and pits, wider gaps (1 is the usual).
    var dry: CGFloat = 1

    var body: some View {
        GeometryReader { geo in
            // The dry streaks are cut out of the mask itself, so they open only this stroke,
            // never whatever is printed under it.
            PrintedInk(rgb: rgb, seed: seed)
                .mask(SlabShape(seed: seed))
                .mask(streaks(geo.size))
        }
        .accessibilityHidden(true)
    }

    private func streaks(_ size: CGSize) -> some View {
        Canvas { ctx, _ in
            var rng = SeededRandom(seed: seed &* 29 &+ 3)
            let w = size.width, h = size.height
            // Ink everywhere (a little past the ragged ends), then the gaps cut out of it.
            ctx.fill(Path(CGRect(x: -20, y: -20, width: w + 40, height: h + 40)), with: .color(.black))
            ctx.blendMode = .destinationOut
            // Every button is the same stroke of the same brush: the streaks are measured from
            // the ends in points, never as a share of the width, so a longer label only makes the
            // middle longer. Most dry streaks hang off the lifting (right) end, a few off the left.
            let reach = min(w, 130)
            for k in 0..<Int(16 * dry) {
                let fromLeft = k % 5 == 4
                // Mostly in the bands above and below the words, so the words stay clean.
                let edge = rng.next() < 0.8
                let y = edge ? (rng.next() < 0.5 ? h * (0.05 + rng.next() * 0.15) : h * (0.8 + rng.next() * 0.15)) : h * (0.25 + rng.next() * 0.5)
                let len = reach * (0.08 + rng.next() * 0.3)
                let start = fromLeft ? 4 + rng.next() * 30 : w + 4 - len - rng.next() * reach * 0.45
                var p = Path()
                p.move(to: CGPoint(x: max(0, start), y: y))
                p.addLine(to: CGPoint(x: min(w + 4, start + len), y: y + (rng.next() - 0.5) * 1.2))
                let open = edge && rng.next() < 0.3 * min(dry, 1.6)
                ctx.stroke(p, with: .color(.black.opacity(open ? 0.9 : edge ? 0.2 + rng.next() * 0.3 * dry : 0.1 + rng.next() * 0.12)),
                           style: StrokeStyle(lineWidth: (open ? 0.9 : 0.5 + rng.next() * 0.5) * (dry > 1 ? 1.3 : 1), lineCap: .round))
            }
            // Pits where the ink ran thin: the same set at each end.
            for k in 0..<Int(22 * dry) {
                let along = rng.next() * 60
                let x = k.isMultiple(of: 3) ? 4 + along * 0.6 : w - 4 - along
                let c = CGPoint(x: x, y: rng.next() < 0.5 ? h * rng.next() * 0.25 : h * (0.75 + rng.next() * 0.25))
                let r = 0.3 + rng.next() * 0.6
                ctx.fill(Path(ellipseIn: CGRect(x: c.x - r, y: c.y - r, width: r * 2, height: r * 2)),
                         with: .color(.black.opacity(0.4 + rng.next() * 0.5)))
            }
        }
    }

    struct SlabShape: Shape {
        var seed: Int
        func path(in r: CGRect) -> Path {
            var rng = SeededRandom(seed: seed)
            let w = r.width, h = r.height
            let p1 = rng.next() * 6.3, p2 = rng.next() * 6.3
            func edge(_ x: CGFloat, _ phase: CGFloat) -> CGFloat { 0.9 * sin(x * 0.05 + phase) + 0.5 * sin(x * 0.17 + phase * 2) }
            var pts: [CGPoint] = []
            // Top edge, left to right, slightly thinner where the brush lifts.
            var x: CGFloat = 5
            while x <= w - 7 {
                let lift = max(0, (x / w - 0.7)) * 3
                pts.append(CGPoint(x: r.minX + x, y: r.minY + 1 + lift + edge(x, p1)))
                x += 4
            }
            // Right end: where the brush lifts. Bristles of every length at uneven spacing,
            // a few long fine ones dragged out past the rest.
            var y = r.minY + 2
            while y < r.maxY - 2 {
                let gap = 1.2 + rng.next() * 2.6
                let long = rng.next() < 0.18
                let reach = long ? 7 + rng.next() * 6 : 1 + rng.next() * 4.5
                let base = r.maxX - 8 + (rng.next() - 0.5) * 3
                pts.append(CGPoint(x: base, y: y))
                pts.append(CGPoint(x: base + reach, y: y + gap * (0.3 + rng.next() * 0.3)))
                y += gap
            }
            // Bottom edge, right to left.
            x = w - 7
            while x >= 5 {
                let lift = max(0, (x / w - 0.7)) * 3
                pts.append(CGPoint(x: r.minX + x, y: r.maxY - 1 - lift + edge(x, p2)))
                x -= 4
            }
            // Left end: where the brush landed. Blunter, bulging a little, a few short frays.
            var yl = r.maxY - 2
            while yl > r.minY + 2 {
                let gap = 1.5 + rng.next() * 3
                let bulge = sin((yl - r.minY) / h * .pi) * 2
                let fray = rng.next() < 0.35 ? 1.5 + rng.next() * 3 : rng.next() * 0.8
                pts.append(CGPoint(x: r.minX + 5 - bulge, y: yl))
                pts.append(CGPoint(x: r.minX + 5 - bulge - fray, y: yl - gap * 0.5))
                yl -= gap
            }
            var p = Path()
            p.move(to: pts[0])
            for q in pts.dropFirst() { p.addLine(to: q) }
            p.closeSubpath()
            return p
        }
    }
}
