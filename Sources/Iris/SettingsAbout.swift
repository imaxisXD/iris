import AppKit
import IrisCore
import SwiftUI

// Settings › About: two sections, like the other pages. Where Iris comes from (the whole iris,
// the name's story in brush, the maker's signature), then Behind Iris in Privacy's rows: a riso
// icon, what it is and one line, and the link that opens in the browser at the right. Report a
// bug isn't here: it's at the rail's foot, on every page.

struct AboutPage: View {
    /// Abhishek's other work: one link, to everything.
    static let moreByMaker = URL(string: "https://github.com/imaxisXD")!
    /// What Iris is built with and their licences: the README's credits.
    static let licences = URL(string: Updater.repo.absoluteString + "#credits")!
    /// The macOS version as people know it ("26.1"), without the build code.
    static var macOS: String {
        let v = ProcessInfo.processInfo.operatingSystemVersion
        return v.patchVersion > 0 ? "\(v.majorVersion).\(v.minorVersion).\(v.patchVersion)" : "\(v.majorVersion).\(v.minorVersion)"
    }

    /// The pause button in the meadow's corner stops the meadow and the story's phrases together.
    @State private var paused = false

    var body: some View {
        VStack(alignment: .leading, spacing: 34) {
            Section(number: 1, title: "Where Iris comes from") {
                VStack(alignment: .leading, spacing: 8) {
                    // The iris in its meadow, quietly alive (the page's one flower), and under it
                    // why Iris exists, in the maker's words. No product named.
                    MakersJourney(paused: $paused).padding(.vertical, 4)
                    // The story and its signature are one centred group, like the headline they copy.
                    VStack(spacing: 14) {
                        MakersNote(paused: paused)
                        // The maker's signature, taped in.
                        Signature()
                            .accessibilityElement(children: .ignore)
                            .accessibilityLabel("Iris is made by Abhishek.")
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 10).padding(.bottom, 6)
                }
            }
            Section(number: 2, title: "Behind Iris") {
                SheetGroup {
                    row(.words, "Free and open source", "MIT licence. Anyone can read how Iris works or suggest a change.") {
                        PenLink(title: "The code", url: Updater.repo)
                    }
                    PenRule(seed: 941)
                    row(.model, "Writes with Gemma 4", "Google's open model, running right on this Mac. Its terms and the fonts' licences are in the credits.") {
                        PenLink(title: "Credits", url: Self.licences)
                    }
                    PenRule(seed: 942)
                    row(.about, "Other work", "Abhishek's other projects, all on GitHub.") {
                        PenLink(title: "More by Abhishek", url: Self.moreByMaker)
                    }
                    PenRule(seed: 943)
                    row(.updates, "This copy", "Iris \(Updater.version) with Gemma 4, on macOS \(Self.macOS).") { EmptyView() }
                }
            }
        }
    }

    /// One fact as Privacy prints them: its riso icon, the fact and one line under it, and what
    /// opens in the browser at the right.
    private func row<Link: View>(_ icon: RisoIcon, _ title: String, _ detail: String, @ViewBuilder link: () -> Link) -> some View {
        HStack(alignment: .top, spacing: DS.Space.md) {
            RisoIconView(icon: icon, size: 26).padding(.top, -3)
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                Text(detail).font(DS.Font.caption).foregroundStyle(DS.Ink.i10).fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            link()
        }
        .accessibilityElement(children: .contain)
    }
}

/// The maker's signature: "made by" in caption and the name on a strip of masking tape, like a
/// zine signed and taped in. Pale, so the name never shouts; the corner's hand word is the
/// page's one loud lettering.
struct Signature: View {
    var body: some View {
        HStack(alignment: .center, spacing: 8) {
            Text("made by").font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
            TapeLabel(text: "Abhishek", size: 11.5)
        }
    }
}

/// Why Iris exists, set as a centred headline with highlighted phrases you can click, like a
/// tool's landing page: each catch writing help came with sits on a soft pill with a sticker
/// poking up from its left end; a click shows the next catch. Letters both phrases share slide
/// to their places, the others fade out and in, the pill tweens to the new width, the old
/// sticker shrinks away and the new one pops past full size and springs back. Never names a
/// product.
struct MakersNote: View {
    /// The story round the catches: Settings' reading size (`DS.Font.subtitle`), so it reads as a
    /// note under the meadow, never a second headline on the page.
    static let wordFont = DS.Font.subtitle
    @MainActor static let wordAdvance: CGFloat = {
        let f = FontRegistry.martian(size: 13.5, weight: 450, width: 87.5)
        return ("M" as NSString).size(withAttributes: [.font: f]).width
    }()
    /// Stopped by the meadow's pause button: the phrases hold their catch.
    var paused = false
    struct Option { let icon: RisoIcon; let words: String }
    enum Piece { case words(String), token([Option], after: String), together(String) }

    static let pieces: [Piece] = [
        .words("I wanted help with my writing that didn’t"),
        .token([Option(icon: .priceTag, words: "cost a monthly fee"),
                Option(icon: .clock, words: "need a subscription"),
                Option(icon: .privacy, words: "lock rewrites behind a plan")], after: ","),
        .words("didn’t send every sentence to"),
        .token([Option(icon: .watchingCloud, words: "someone else’s server"),
                Option(icon: .model, words: "a stranger’s computer"),
                Option(icon: .tray, words: "a company’s data center")], after: ","),
        .words("and didn’t"),
        .token([Option(icon: .wifi, words: "go quiet offline"),
                Option(icon: .paperPlane, words: "give up on a plane"),
                Option(icon: .clock, words: "stall on slow Wi-Fi")], after: "."),
        .together("So I made Iris."),
    ]

    /// Which highlighted phrase this is (0, 1, 2), so they take turns changing.
    static func order(of piece: Int) -> Int {
        pieces.prefix(piece).filter { if case .token = $0 { return true } else { return false } }.count
    }

    var body: some View {
        VStack(spacing: 12) {
            Flow(spacing: 7, lineSpacing: 8, centred: true) {
                ForEach(Array(Self.pieces.enumerated()), id: \.offset) { i, piece in
                    switch piece {
                    case .words(let text):
                        ForEach(Array(text.split(separator: " ").enumerated()), id: \.offset) { _, w in
                            Text(String(w)).font(Self.wordFont).foregroundStyle(DS.Ink.i11).fixedSize()
                        }
                    case .together(let text):
                        // A short sentence that never breaks across lines.
                        Text(text).font(Self.wordFont).foregroundStyle(DS.Ink.i11).fixedSize()
                    case .token(let options, let after):
                        // The comma or full stop sits right on the pill's end, never a space off it.
                        HStack(spacing: -3) {
                            Token(options: options, seed: i, order: Self.order(of: i), paused: paused)
                            Text(after).font(Self.wordFont).foregroundStyle(DS.Ink.i11)
                        }
                        .fixedSize()
                        .layoutValue(key: ReservedWidth.self, value: Token.width(longest: options) - 3 + Self.wordAdvance * CGFloat(after.count))
                    }
                }
            }
        }
        .frame(maxWidth: .infinity)
    }
}

/// One highlighted phrase. Its moves are worked out from the time since the last click, so
/// every frame is the same each time (timings measured off the reference).
private struct Token: View {
    let options: [MakersNote.Option]
    let seed: Int
    /// Its turn among the phrases: they change one after another, never together.
    let order: Int
    let paused: Bool
    @State private var index = 0
    @State private var previous = 0
    @State private var changed = Date.distantPast
    @State private var hover = false
    @State private var moving = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// A catch is set a size up from the story round it (Martian 16.5 against 13.5, the same one
    /// weight), so the three catches and their characters stand out and the rest stays a note.
    static let size: CGFloat = 16.5
    static let font = DS.Font.martian(size)
    static let tracking: CGFloat = 0
    /// A pill one line tall, like the reference's.
    static let height: CGFloat = 27
    /// Martian is monospaced: every letter takes this much room, tracking included.
    @MainActor static let advance: CGFloat = {
        let f = FontRegistry.martian(size: size, weight: 450, width: 87.5)
        return ("M" as NSString).size(withAttributes: [.font: f]).width + tracking
    }()

    /// The morph: 200 ms on an expo-out curve; letters leave in 50 ms, arrive in 100 ms after 50.
    static let move = 0.2, leave = 0.05, arriveDelay = 0.05, arrive = 0.1
    /// The sticker: rests tilted 4°; the old one shrinks to half and turns −14° as it fades
    /// (200 ms); the new one fades in (180 ms), grows 65% → 120% while turning −5° → 9°
    /// (380 ms), then springs back to full size at 4°.
    static let rest = 4.0, grow = 0.38
    /// Changing on its own: the first change after this long, the phrases this far apart, then
    /// each changes again after `cycle` (three phrases, so one changes every 1.5 s or so).
    static let firstTurn = 2.0, turnGap = 1.5, cycle = 4.5

    /// How wide the pill is at its longest option: the sticker's room, the letters, the padding.
    @MainActor static func width(longest options: [MakersNote.Option]) -> CGFloat {
        2.5 + (height + 8) + CGFloat(options.map(\.words.count).max() ?? 0) * advance + 6
    }

    var body: some View {
        // A real button: Tab reaches it, Space and Return press it, and it wears the pen's
        // focus ring and the press squeeze every Iris button has.
        Button { step() } label: {
            TimelineView(.animation(paused: reduceMotion || !moving)) { ctx in
                let t = reduceMotion ? 9 : ctx.date.timeIntervalSince(changed)
                content(t)
            }
            .contentShape(Capsule())
        }
        .buttonStyle(SqueezeStyle(scale: 0.96))
        .onHover { hover = $0 }
        // Frames only while a swap plays.
        .task(id: changed) {
            guard changed != .distantPast else { return }
            moving = true
            try? await Task.sleep(for: .seconds(0.9))
            moving = false
        }
        // On its own: each phrase shows its next catch every few seconds, the three taking
        // turns. It holds while the pointer is over it, while paused, and under Reduce Motion;
        // a click shows the next one at once and starts its wait again.
        .task(id: "\(changed.timeIntervalSinceReferenceDate)-\(paused)-\(hover)-\(reduceMotion)") {
            // On the test display (`OnDisplay`) it holds its first catch, so a capture is of one moment.
            guard !paused, !hover, !reduceMotion, !OnDisplay.isOn else { return }
            let first = changed == .distantPast
            try? await Task.sleep(for: .seconds(first ? Self.firstTurn + Double(order) * Self.turnGap : Self.cycle))
            guard !Task.isCancelled else { return }
            step()
        }
        .pointerStyle(.link)
        .accessibilityLabel(options[index].words)
        .accessibilityHint("Shows another catch: \(options[(index + 1) % options.count].words)")
    }

    private func step() {
        previous = index
        index = (index + 1) % options.count
        changed = Date()
    }

    private func content(_ t: Double) -> some View {
        let from = Array(options[previous].words), to = Array(options[index].words)
        let k = Self.expoOut(t / Self.move)
        let oldW = CGFloat(from.count) * Self.advance, newW = CGFloat(to.count) * Self.advance
        let width = oldW + (newW - oldW) * CGFloat(k)
        return HStack(spacing: 0) {
            stickers(t).frame(width: Self.height + 8, height: Self.height)
            ZStack(alignment: .topLeading) {
                ForEach(Self.letters(from: from, to: to), id: \.id) { l in letter(l, t, k) }
            }
            .frame(width: max(width, 1), height: Self.height, alignment: .leading)
        }
        .padding(.leading, 2.5).padding(.trailing, 6)
        .background(Capsule().printedInk(hover ? (230, 222, 216) : (236, 230, 225), seed: 500 + seed))
    }

    private func letter(_ l: Letter, _ t: Double, _ k: Double) -> some View {
        let a = Self.advance
        var x: CGFloat, scale: CGFloat = 1, opacity: Double = 1
        switch (l.fromIndex, l.toIndex) {
        case let (f?, n?):                       // shared: slides
            x = (CGFloat(f) + CGFloat(n - f) * CGFloat(k)) * a
        case let (nil, n?):                      // arriving: travels with its neighbours, fades in
            x = (CGFloat(n) + CGFloat(l.shift) * CGFloat(1 - k)) * a
            scale = 0.95 + 0.05 * CGFloat(k)
            opacity = Curve.clamp((t - Self.arriveDelay) / Self.arrive)
        case let (f?, nil):                      // leaving: travels with its neighbours, fades out
            x = (CGFloat(f) + CGFloat(l.shift) * CGFloat(k)) * a
            scale = 1 - 0.05 * CGFloat(k)
            opacity = 1 - Curve.clamp(t / Self.leave)
        default: x = 0
        }
        return Text(String(l.char)).font(Self.font).foregroundStyle(DS.Ink.i12).fixedSize()
            .scaleEffect(scale)
            .opacity(opacity)
            .frame(height: Self.height)
            .offset(x: x)
    }

    private func stickers(_ t: Double) -> some View {
        let lift = hover ? 1.08 : 1.0
        // Leaving.
        let out = Curve.clamp(t / 0.2)
        let oldScale = 1 - 0.5 * Curve.easeOut(t / 0.3)
        let oldTurn = Self.rest + (-14 - Self.rest) * Curve.easeOut(t / 0.3)
        // Arriving.
        let inScale: Double, inTurn: Double
        if t < Self.grow {
            let e = Curve.easeOut(t / Self.grow)
            inScale = 0.65 + 0.55 * e; inTurn = -5 + 14 * e
        } else {
            let d = t - Self.grow
            let ring = exp(-d * 13) * cos(d * 28)
            inScale = 1 + 0.2 * ring; inTurn = Self.rest + 5 * ring
        }
        return ZStack {
            if t < 0.3 && previous != index {
                Sticker(icon: options[previous].icon, since: .distantPast).scaleEffect(oldScale).rotationEffect(.degrees(oldTurn)).opacity(1 - out)
            }
            Sticker(icon: options[index].icon, since: changed)
                .scaleEffect(inScale * lift)
                .rotationEffect(.degrees(inTurn))
                .opacity(Curve.clamp(t / 0.18))
        }
        .offset(y: -4)
    }

    /// expo-out, as cubic-bezier(0.19, 1, 0.22, 1) reads.
    static func expoOut(_ x: Double) -> Double { let c = Curve.clamp(x); return c >= 1 ? 1 : 1 - pow(2, -10 * c) }

    struct Letter { let id: String; let char: Character; let fromIndex: Int?; let toIndex: Int?
        /// How far its nearest shared neighbour moves, so unmatched letters travel with the crowd.
        let shift: Int }

    static func letters(from: [Character], to: [Character]) -> [Letter] {
        var used = Set<Int>(), pairs: [(Int, Int)] = []
        for (j, c) in to.enumerated() where c != " " {
            if let m = from.indices.filter({ from[$0] == c && !used.contains($0) }).min(by: { abs($0 - j) < abs($1 - j) }) {
                used.insert(m); pairs.append((m, j))
            }
        }
        func shiftNear(_ i: Int, inFrom: Bool) -> Int {
            guard let p = pairs.min(by: { abs((inFrom ? $0.0 : $0.1) - i) < abs((inFrom ? $1.0 : $1.1) - i) }) else { return 0 }
            return p.1 - p.0
        }
        var out: [Letter] = pairs.map { Letter(id: "m\($0.0)", char: from[$0.0], fromIndex: $0.0, toIndex: $0.1, shift: 0) }
        let matchedTo = Set(pairs.map(\.1))
        for (j, c) in to.enumerated() where !matchedTo.contains(j) {
            out.append(Letter(id: "n\(j)-\(String(to))", char: c, fromIndex: nil, toIndex: j, shift: -shiftNear(j, inFrom: false)))
        }
        for (i, c) in from.enumerated() where !used.contains(i) {
            out.append(Letter(id: "o\(i)-\(String(from))", char: c, fromIndex: i, toIndex: nil, shift: shiftNear(i, inFrom: true)))
        }
        return out
    }
}

/// The catch's icon on the pill's end: a blue crayon creature (`ScribbleCritter`).
private struct Sticker: View {
    let icon: RisoIcon
    let since: Date
    var body: some View {
        ScribbleCritter(icon: icon, size: 40, since: since)
    }
}

/// The widest a view in a `Flow` can become (a phrase's longest option), for breaking lines.
private struct ReservedWidth: LayoutValueKey {
    static let defaultValue: CGFloat = 0
}

/// Lays its views out left to right, wrapping onto a new line when the next won't fit.
private struct Flow: Layout {
    var spacing: CGFloat = 6
    var lineSpacing: CGFloat = 6
    var centred = false

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = arrange(width: proposal.width ?? .infinity, subviews)
        let width = rows.map { $0.width }.max() ?? 0
        let height = rows.reduce(0) { $0 + $1.height } + CGFloat(max(rows.count - 1, 0)) * lineSpacing
        return CGSize(width: proposal.width ?? width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for row in arrange(width: bounds.width, subviews) {
            var x = centred ? bounds.minX + (bounds.width - row.width) / 2 : bounds.minX
            for i in row.items {
                let size = subviews[i].sizeThatFits(.unspecified)
                subviews[i].place(at: CGPoint(x: x, y: y + (row.height - size.height) / 2), proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += row.height + lineSpacing
        }
    }

    private struct Row { var items: [Int] = []; var width: CGFloat = 0; var height: CGFloat = 0 }

    /// Rows are broken as if every view were as wide as it may ever get (`ReservedWidth`), so a
    /// phrase changing never moves words onto another line; each row is then centred on the
    /// width its views have now.
    private func arrange(width: CGFloat, _ subviews: Subviews) -> [Row] {
        var rows: [Row] = [Row()]
        var reserved: CGFloat = 0
        for (i, view) in subviews.enumerated() {
            let size = view.sizeThatFits(.unspecified)
            let room = max(size.width, view[ReservedWidth.self])
            let extra = rows[rows.count - 1].items.isEmpty ? room : room + spacing
            if reserved + extra > width, !rows[rows.count - 1].items.isEmpty {
                rows.append(Row())
                reserved = 0
            }
            let isFirst = rows[rows.count - 1].items.isEmpty
            reserved += isFirst ? room : room + spacing
            rows[rows.count - 1].items.append(i)
            rows[rows.count - 1].width += isFirst ? size.width : size.width + spacing
            rows[rows.count - 1].height = max(rows[rows.count - 1].height, size.height)
        }
        return rows
    }
}
