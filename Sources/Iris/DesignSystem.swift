import AppKit
import CoreText
import SwiftUI

// Iris design system (light only; the app pins NSApp.appearance to .aqua): the riso
// palette and type from the coffee zine (DESIGN.md, "Where the look comes from"), with the
// tactile-design rules (.claude/skills/tactile-design) for layering, edges, drops,
// press, hover, focus and motion. Components use roles (ink-N, surfaces), never hex.

/// Bytes of one riso ink, for printing it through the screen.
typealias RGB8 = (UInt8, UInt8, UInt8)

/// Every riso ink Iris prints with, defined once. `DS.Hue` and ink-12 are these as colours;
/// printed areas (`printedInk`), the icons (`RisoArt`), the plates, the iris rig and the menu
/// bar flower take the bytes.
enum Riso {
    /// Iris's mark: inserted words, the recording state, the plate off register.
    static let pink: RGB8 = (240, 134, 176)
    static let pinkTint: RGB8 = (252, 228, 238)
    static let pinkText: RGB8 = (142, 36, 83)
    /// A hotter pink for the big screen-printed plates (a page's foot).
    static let platePink: RGB8 = (236, 110, 176)
    /// The pink settled toward espresso: what shows through the primary button's dry brush.
    static let rose: RGB8 = (170, 102, 124)
    /// Removed words, and the icons' line work.
    static let espresso: RGB8 = (100, 56, 46)
    /// Ink-12: type, keycaps and the darkest ink in the drawings, a warm espresso black.
    static let ink: RGB8 = (42, 26, 22)
    /// The red pen: problems, and what Delete would destroy.
    static let red: RGB8 = (217, 67, 46)
    /// Keycaps, and the icons' shade plate.
    static let crema: RGB8 = (226, 200, 185)
    static let tan: RGB8 = (196, 157, 122)
    static let sky: RGB8 = (120, 162, 220)
    /// The garden's sun, so it doesn't melt into the pink flower.
    static let sunflower: RGB8 = (255, 190, 40)
    /// The garden's butterflies, warm against the blue sky and the pink bloom.
    static let orange: RGB8 = (255, 116, 60)
    /// The paper everything is printed on: cards, overlays, windows.
    static let paper: RGB8 = (255, 252, 249)
}

enum DS {
    /// A riso ink as a colour.
    static func color(_ ink: RGB8) -> Color { Color(nsColor: nsColor(ink)) }
    static func nsColor(_ ink: RGB8) -> NSColor {
        NSColor(srgbRed: CGFloat(ink.0) / 255, green: CGFloat(ink.1) / 255, blue: CGFloat(ink.2) / 255, alpha: 1)
    }

    // MARK: Neutral ladder (ink-1 … ink-12)
    //
    // Riso paper to riso ink. Each step has the tactile job: 1–4 grounds and fills, 8 lines,
    // 9–10 faint text, 11 secondary text, 12 strong text and the primary fill.
    enum Ink {
        static let i1 = hex(0xF7F3F0)
        static let i2 = hex(0xF0EBE6)
        static let i3 = hex(0xE9E2DC)
        static let i4 = hex(0xE1D9D2)
        static let i8 = hex(0xA3958B)
        static let i9 = hex(0x84766D)
        static let i10 = hex(0x6B5F58)  // meta: 5.2:1 on paper
        static let i11 = hex(0x4A403B)
        // Warm espresso black: type in the same ink as the drawings, like the zine.
        static let i12 = DS.color(Riso.ink)
    }

    // MARK: Surface roles

    enum Surface {
        /// The window behind everything (settings rail, the table under a sheet).
        static let ground = hex(0xF0EBE6)
        /// The page body: riso paper.
        static let panel = Ink.i1
        /// Raised cards and controls: a fresher sheet, a touch lighter than paper.
        static let top = DS.color(Riso.paper)
        /// Controls: a fresh sheet, like cards.
        static let control = DS.color(Riso.paper)
        static let controlHover = hex(0xF7F3F0)
        /// Overlays (menus, the panel, sheets).
        static let overlay = DS.color(Riso.paper)
        /// Fields and toggle tracks sink.
        static let well = Ink.i3
        static let wellFocus = Ink.i4
    }

    // MARK: Meaning hues (riso inks). Hue carries meaning, never rank.

    enum Hue {
        /// Iris's mark: inserted words, the recording state.
        static let pink = DS.color(Riso.pink)
        static let pinkText = DS.color(Riso.pinkText)
        /// Removed words.
        static let espresso = DS.color(Riso.espresso)
        /// Warning (a big rewrite): crema tint with espresso text.
        static let warnTint = hex(0xF4E7DC)
        static let warnText = hex(0x6E3F22)
        /// Danger: riso red.
        static let red = DS.color(Riso.red)
        static let redTint = hex(0xFBE6E1)
        static let redText = hex(0xA52A18)
        /// Riso green, for one mark only: the tick by "looks good" when there's nothing to fix.
        static let green = hex(0x1E8C55)
        /// Masking tape: a pale tan, printed with multiply so the paper's grain shows through.
        static let tape = hex(0xE0D1BB)
    }

    static let primaryText = Ink.i1
    static let focusRing = Ink.i10

    // MARK: Shadow ink

    /// Drops are drawn in ink-12, never lighter than the parent.
    static func drop(_ alpha: Double) -> Color {
        Color(nsColor: NSColor(srgbRed: 8 / 255, green: 6 / 255, blue: 5 / 255, alpha: alpha))
    }

    // MARK: Radii (control set to 4: riso paper slips are sharp)

    enum Radius {
        static let control: CGFloat = 4
    }

    // MARK: Sizes

    enum Size {
        static let control: CGFloat = 32  // desktop control height for a 12.5/20 face
        static let pill: CGFloat = 26
        static let icon: CGFloat = 22
        static let row: CGFloat = 38
    }

    enum Space {
        static let xs: CGFloat = 4
        static let sm: CGFloat = 8
        static let md: CGFloat = 12
        static let lg: CGFloat = 16
    }

    // MARK: Motion
    //
    // One set of timings for the whole app. Each tier pairs an entrance with an exit one
    // step quicker, so dismissals read final instead of replaying the entrance backwards.
    // Magnitudes are the tactile-design skill's; the pairing is Fluid Functionalism's.
    // Under Reduce Motion, travel and scale are dropped and fades stay.

    enum Motion {
        struct Tier {
            let enter: Double
            let exit: Double
            var enterAnimation: Animation { .timingCurve(0.23, 1, 0.32, 1, duration: enter) }
            var exitAnimation: Animation { .timingCurve(0.23, 1, 0.32, 1, duration: exit) }
        }
        /// Hover, press, colour, menus, toasts, the panel fade.
        static let fast = Tier(enter: 0.15, exit: 0.12)
        /// Sheets and dialogs.
        static let slow = Tier(enter: 0.24, exit: 0.20)
        /// The segmented toggle's sliding pill: overshoot stays inside the track's inset.
        static let pill = Animation.spring(duration: 0.4, bounce: 0.12)
        /// The switch knob.
        static let knob = Animation.spring(duration: 0.45, bounce: 0.2)
        /// How long a toast stays readable before it leaves.
        static let toastHold: Duration = .seconds(2.4)
    }

    /// Kept for call sites: the fast tier's entrance.
    static let easeOut = Motion.fast.enterAnimation

    // MARK: Type
    //
    // One UI face, Martian Mono, at one weight (450) and 87.5% width. Hierarchy is a size
    // step plus an ink step. The condensed caps are the display face; Caveat Brush is for
    // names and headings only, never inside controls; the doodle face is the name "Iris".

    enum Font {
        // The UI roles, each one size; every UI string uses one of these.
        static let micro = martian(9.5)       // tiny numbers: segment indexes, rail folios, captions
        static let caption = martian(11)      // meta, descriptions, table cells, key hints
        static let body = martian(12.5)       // controls and values
        static let subtitle = martian(13.5)   // the message in the panel
        /// The poster's light folio numbers ("/03").
        static let folio = martian(13.5, weight: 300)
        /// Legends on keycaps and key chips.
        static let key = martian(10.5, weight: 560)
        static func display(_ size: CGFloat) -> SwiftUI.Font { martian(size, weight: 400, width: 75) }
        static func brush(_ size: CGFloat) -> SwiftUI.Font { .custom(Name.brush, size: size) }
        static func logo(_ size: CGFloat) -> SwiftUI.Font { .custom(Name.logo, size: size) }
        /// Chunky cut-paper caps for a ghost word behind a page (the zine's CRUSH). Never for text you read.
        static func chunky(_ size: CGFloat) -> SwiftUI.Font { .custom(Name.chunky, size: size) }
        /// The bundled faces (Resources/Fonts), by the names SwiftUI and Core Text know them.
        enum Name {
            static let brush = "Caveat Brush", logo = "Rubik Doodle Shadow", chunky = "LondrinaSolid-Black"
            /// The hand-lettered words (`HandWord`): Knewave for the fat letters, Mansalva for the sketched.
            static let fat = "Knewave-Regular", sketch = "Mansalva-Regular"
        }
        static func martian(_ size: CGFloat, weight: CGFloat = 450, width: CGFloat = 87.5) -> SwiftUI.Font {
            SwiftUI.Font(FontRegistry.martian(size: size, weight: weight, width: width))
        }
    }

    // MARK: Helpers

    static func hex(_ value: UInt32) -> Color { Color(nsColor: rgb(value)) }

    static func rgb(_ hex: UInt32, alpha: CGFloat = 1) -> NSColor {
        NSColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
                blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
    }
}

// MARK: - Fonts

enum FontRegistry {
    private static var registered = false
    private static var cache: [String: NSFont] = [:]

    /// Registers the bundled OFL fonts for this process. Safe to call more than once.
    static func registerAll() {
        guard !registered else { return }
        registered = true
        guard let dir = Bundle.module.url(forResource: "Fonts", withExtension: nil),
            let files = try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)
        else { return }
        let fonts = files.filter { $0.pathExtension == "ttf" } as CFArray
        CTFontManagerRegisterFontURLs(fonts, .process, true, nil)
    }

    /// Martian Mono at an exact weight and width from its variable axes.
    static func martian(size: CGFloat, weight: CGFloat, width: CGFloat) -> NSFont {
        registerAll()
        let key = "\(size)-\(weight)-\(width)"
        if let font = cache[key] { return font }
        let wght = 0x7767_6874, wdth = 0x7764_7468  // 'wght', 'wdth'
        let attributes: [NSFontDescriptor.AttributeName: Any] = [
            .family: "Martian Mono",
            NSFontDescriptor.AttributeName(rawValue: kCTFontVariationAttribute as String): [wght: weight, wdth: width],
        ]
        let font = NSFont(descriptor: NSFontDescriptor(fontAttributes: attributes), size: size)
            ?? NSFont.monospacedSystemFont(ofSize: size, weight: .medium)
        cache[key] = font
        return font
    }
}

// MARK: - Shapes

/// A rectangle drawn as one slightly wobbly stroke, seeded so it never re-jitters.
/// The riso hand: keycaps, buttons, and (with the shapes below) the panel's rules and red pen.
struct HandRect: Shape {
    var seed: Int
    var wobble: CGFloat = 0.45
    var overshoot: CGFloat = 2.5
    var closed = false

    func path(in rect: CGRect) -> Path {
        var rng = SeededRandom(seed: seed)
        let w = rect.width, h = rect.height
        let per = 2 * (w + h)
        let p1 = rng.next() * 6.3, p2 = rng.next() * 6.3
        let f1 = 0.05 + rng.next() * 0.04, f2 = 0.14 + rng.next() * 0.06
        func at(_ d: CGFloat) -> (CGPoint, CGVector) {
            var d = d.truncatingRemainder(dividingBy: per)
            if d < 0 { d += per }
            if d < w { return (CGPoint(x: rect.minX + d, y: rect.minY), CGVector(dx: 0, dy: -1)) }
            d -= w
            if d < h { return (CGPoint(x: rect.maxX, y: rect.minY + d), CGVector(dx: 1, dy: 0)) }
            d -= h
            if d < w { return (CGPoint(x: rect.maxX - d, y: rect.maxY), CGVector(dx: 0, dy: 1)) }
            d -= w
            return (CGPoint(x: rect.minX, y: rect.maxY - d), CGVector(dx: -1, dy: 0))
        }
        var points: [CGPoint] = []
        let start = -1 - rng.next() * 1.5
        let end = per + (closed ? 0 : overshoot)
        var d = start
        while d <= end {
            let (p, n) = at(d)
            let offset = wobble * (sin(d * f1 + p1) * 0.6 + sin(d * f2 + p2) * 0.4)
            points.append(CGPoint(x: p.x + n.dx * offset, y: p.y + n.dy * offset))
            d += 5
        }
        return Path.catmullRom(points, closed: closed)
    }
}

/// A horizontal rule drawn with the keycaps' hand: one slightly wobbly ink stroke.
struct HandLine: Shape {
    var seed: Int
    var wobble: CGFloat = 0.7
    /// Runs top to bottom instead of left to right.
    var vertical = false

    func path(in rect: CGRect) -> Path {
        var rng = SeededRandom(seed: seed)
        let p1 = rng.next() * 6.3, p2 = rng.next() * 6.3
        let f1 = 0.015 + rng.next() * 0.015, f2 = 0.06 + rng.next() * 0.04
        let length = vertical ? rect.height : rect.width
        let start = vertical ? rect.minY : rect.minX
        var points: [CGPoint] = []
        var d = start + rng.next() * 2
        let end = start + length - rng.next() * 2
        while d <= end {
            let offset = wobble * (sin(d * f1 + p1) * 0.6 + sin(d * f2 + p2) * 0.4)
            points.append(vertical ? CGPoint(x: rect.midX + offset, y: d) : CGPoint(x: d, y: rect.midY + offset))
            d += 6
        }
        return Path.catmullRom(points, closed: false)
    }
}

/// A pen tick, the way a checklist gets marked off by hand.
struct HandTick: Shape {
    func path(in rect: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: rect.minX, y: rect.minY + rect.height * 0.55))
        p.addQuadCurve(to: CGPoint(x: rect.minX + rect.width * 0.36, y: rect.maxY),
                       control: CGPoint(x: rect.minX + rect.width * 0.2, y: rect.minY + rect.height * 0.75))
        p.addQuadCurve(to: CGPoint(x: rect.maxX, y: rect.minY),
                       control: CGPoint(x: rect.minX + rect.width * 0.55, y: rect.minY + rect.height * 0.35))
        return p
    }
}

/// A small pen chevron for pickers.
struct HandChevron: Shape {
    func path(in rect: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: rect.minX, y: rect.minY))
        p.addQuadCurve(to: CGPoint(x: rect.midX, y: rect.maxY), control: CGPoint(x: rect.midX * 0.9, y: rect.midY * 1.2))
        p.addLine(to: CGPoint(x: rect.maxX, y: rect.minY + 0.5))
        return p
    }
}

extension View {
    /// The one hover (and "you are here") look: a pen-drawn wash of grey ink behind the
    /// thing, like the Settings rail. Hover is `DS.Ink.i3`, current is a step darker. Never
    /// a flat or rounded rectangle, and never pink: pink means chosen.
    func penWash(hover: Bool, current: Bool = false, seed: Int) -> some View {
        background(HandRect(seed: seed, wobble: 0.5, closed: true)
            .fill(current ? DS.Ink.i4 : (hover ? DS.Ink.i3 : .clear)))
    }

    /// A sheet of paper lying on the table: a long soft shadow pulled in at the sides so it
    /// pools below, plus a hairline of contact. Square corners, like the panel.
    func paperShadow(spread: CGFloat, drop: CGFloat, opacity: Double = 0.18) -> some View {
        background {
            ZStack {
                Rectangle().fill(Color.black.opacity(opacity)).padding(.horizontal, spread).offset(y: drop).blur(radius: drop)
                Rectangle().fill(Color.black.opacity(0.10)).offset(y: 1).blur(radius: 0.6)
            }
            .allowsHitTesting(false)
        }
    }
}

/// A divider drawn with the pen, so every line in Iris is the same hand as the keycaps.
/// The seed defaults to the caller's line number, so stacked rules don't wobble in step.
struct PenRule: View {
    var seed: Int
    var ink: Color
    var width: CGFloat

    init(seed: Int = #line, ink: Color = DS.Ink.i12.opacity(0.3), width: CGFloat = 1) {
        self.seed = seed
        self.ink = ink
        self.width = width
    }

    var body: some View {
        HandLine(seed: seed, wobble: 0.5).stroke(ink, style: StrokeStyle(lineWidth: width, lineCap: .round))
            .frame(height: 3)
            .accessibilityHidden(true)
    }
}

/// An editor's pen loop: one stroke round the word that overshoots its start.
struct HandEllipse: Shape {
    var seed: Int
    var wobble: CGFloat = 1.1

    func path(in rect: CGRect) -> Path {
        var rng = SeededRandom(seed: seed)
        let start = -2.5 + rng.next() * 0.3, sweep = 2 * CGFloat.pi + 0.55, phase = rng.next() * 6.3
        var points: [CGPoint] = []
        var t: CGFloat = 0
        while t <= 1 {
            let a = start + sweep * t
            let grow = 1 + 0.1 * t  // a slight spiral, so the ends pass each other like a real pen loop
            let o = wobble * sin(a * 3 + phase)
            points.append(CGPoint(x: rect.midX + (rect.width / 2 * grow + o) * cos(a),
                                  y: rect.midY + (rect.height / 2 * grow + o) * sin(a)))
            t += 1.0 / 40
        }
        return Path.catmullRom(points, closed: false)
    }
}

/// A quick pen arrow for marking up, like an editor's: `.left` curves up and points right
/// (flip it to point back at a circled word); `.down` curls over and points down, at a
/// button below the note. Only for pointing at a thing on the page (`HandNote`, the panel's
/// red pen). Never on a button or link: those take the printed `PenArrow`.
struct HandArrow: Shape {
    enum Curl { case left, down }
    var curl: Curl = .left

    func path(in rect: CGRect) -> Path {
        var p = Path()
        switch curl {
        case .left:
            let a = CGPoint(x: rect.minX, y: rect.maxY - 2), b = CGPoint(x: rect.maxX, y: rect.minY + rect.height * 0.45)
            p.move(to: a)
            p.addQuadCurve(to: b, control: CGPoint(x: rect.midX, y: rect.minY - 4))
            p.move(to: CGPoint(x: b.x - 7, y: b.y - 5))
            p.addLine(to: b)
            p.addLine(to: CGPoint(x: b.x - 6, y: b.y + 5))
        case .down:
            let a = CGPoint(x: rect.minX, y: rect.minY + 3), b = CGPoint(x: rect.maxX - 3, y: rect.maxY)
            p.move(to: a)
            p.addQuadCurve(to: b, control: CGPoint(x: rect.maxX, y: rect.minY))
            p.move(to: CGPoint(x: b.x - 5, y: b.y - 7))
            p.addLine(to: b)
            p.addLine(to: CGPoint(x: b.x + 5, y: b.y - 6))
        }
        return p
    }
}

extension Path {
    static func catmullRom(_ p: [CGPoint], closed: Bool) -> Path {
        var path = Path()
        guard let first = p.first else { return path }
        path.move(to: first)
        for i in 0..<(p.count - 1) {
            let a = i > 0 ? p[i - 1] : p[i], b = p[i], c = p[i + 1], e = i + 2 < p.count ? p[i + 2] : c
            path.addCurve(to: c,
                          control1: CGPoint(x: b.x + (c.x - a.x) / 6, y: b.y + (c.y - a.y) / 6),
                          control2: CGPoint(x: c.x - (e.x - b.x) / 6, y: c.y - (e.y - b.y) / 6))
        }
        if closed { path.closeSubpath() }
        return path
    }
}

struct SeededRandom {
    private var state: UInt64
    init(seed: Int) { state = UInt64(truncatingIfNeeded: seed &* 2_654_435_761 &+ 1) | 1 }
    mutating func next() -> CGFloat {
        state = state &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
        return CGFloat(state >> 33) / CGFloat(UInt64(1) << 31)
    }
}

extension View {
    /// The one focus treatment for every control: a 2px pen line drawn round it, 4px outside,
    /// never a machine-drawn rounded rectangle.
    func focusRing(_ visible: Bool) -> some View {
        overlay {
            HandRect(seed: 61, wobble: 0.6, overshoot: 4)
                .stroke(DS.focusRing, style: StrokeStyle(lineWidth: 2, lineCap: .round, lineJoin: .round))
                .padding(-4)
                .opacity(visible ? 1 : 0)
                .allowsHitTesting(false)
        }
    }

    /// Riso paper tooth over a surface.
    func grain(_ opacity: Double = 0.22) -> some View {
        overlay {
            // Texture, not a picture: hidden from VoiceOver, or it stands in front of everything
            // under it when something asks what's at a point (VoiceOver following the pointer).
            Image(nsImage: Grain.tile).resizable(resizingMode: .tile)
                .opacity(opacity).blendMode(.multiply).allowsHitTesting(false)
                .accessibilityHidden(true)
        }
    }
}

/// A small tiling noise texture, generated once.
enum Grain {
    static let tile: NSImage = {
        let size = 128
        var rng = SeededRandom(seed: 42)
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
                                   samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                                   bytesPerRow: size * 4, bitsPerPixel: 32)!
        let data = rep.bitmapData!
        for i in 0..<(size * size) {
            let v = rng.next()
            let alpha = UInt8(max(0, min(255, (v - 0.35) * 170)))
            data[i * 4] = 26; data[i * 4 + 1] = 20; data[i * 4 + 2] = 16; data[i * 4 + 3] = alpha
        }
        let image = NSImage(size: NSSize(width: size / 2, height: size / 2))
        image.addRepresentation(rep)
        return image
    }()
}
