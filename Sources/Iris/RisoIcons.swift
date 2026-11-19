import AppKit
import SwiftUI

/// Iris's own icons, printed like the logo's flower: a crema shadow plate, a pink plate and
/// an espresso ink plate, each dithered and a pixel or two off register. Drawn on an
/// 80×80 canvas (top-left origin) and printed small, so they read as riso prints rather
/// than vector glyphs.
enum RisoIcon: String, CaseIterable {
    case general, shortcuts, tones, words, apps, model, privacy, updates, about, clock, hand, paperPlane, tray, bug, priceTag, watchingCloud, wifi, wand

    /// Printed once per size and kept, so an icon never re-dithers between renders.
    @MainActor func image() -> NSImage {
        printed(pixels: 44, offsets: ((2, 2), (-1, -1)), key: rawValue)
    }

    /// The icon printed big, as a drawing rather than an icon: the pink plate lands a few dots
    /// off the ink (`RisoDrawing`), the way the zine's brewers are printed.
    @MainActor func poster(pixels: Int) -> NSImage {
        let off = max(2, pixels / 28)
        return printed(pixels: pixels, offsets: ((off, off + 1), (off + 1, -off)), key: "poster-\(rawValue)-\(pixels)")
    }

    /// The three plates: crema shade and pink at `offsets`, the espresso ink on its mark.
    @MainActor private func printed(pixels: Int, offsets: (shade: (Int, Int), pink: (Int, Int)), key: String) -> NSImage {
        if let cached = Self.cache[key] { return cached }
        let s = CGFloat(pixels) / 80
        let art = art
        let scaled: (@escaping (CGContext) -> Void) -> (CGContext) -> Void = { draw in
            { ctx in ctx.scaleBy(x: s, y: s); draw(ctx) }
        }
        let image = RisoArt.print(pixels: pixels, seed: seed, plates: [
            (Riso.crema, offsets.shade.0, offsets.shade.1, scaled(art.shade)),
            (Riso.pink, offsets.pink.0, offsets.pink.1, scaled(art.pink)),
            (Riso.espresso, 0, 0, scaled(art.ink)),
        ])
        Self.cache[key] = image
        return image
    }

    @MainActor private static var cache: [String: NSImage] = [:]
    private var seed: Int { rawValue.unicodeScalars.reduce(11) { $0 &* 31 &+ Int($1.value) } }

    private struct Art {
        let shade: (CGContext) -> Void
        let pink: (CGContext) -> Void
        let ink: (CGContext) -> Void
    }

    private var art: Art {
        switch self {
        case .general: Self.sliders
        case .shortcuts: Self.commandKey
        case .tones: Self.bubble
        case .words: Self.book
        case .apps: Self.tiles
        case .model: Self.chip
        case .privacy: Self.lock
        case .updates: Self.sprout
        case .about: Self.nib
        case .clock: Self.alarmClock
        case .hand: Self.palm
        case .paperPlane: Self.plane
        case .tray: Self.emptyTray
        case .bug: Self.ladybird
        case .priceTag: Self.tag
        case .watchingCloud: Self.cloudEye
        case .wifi: Self.signal
        case .wand: Self.magicWand
        }
    }

    // MARK: Drawings

    /// General (and Settings… in the menu): three sliders at different settings, their knobs pink.
    private static let sliders: Art = {
        let rows: [(y: CGFloat, x: CGFloat)] = [(20, 52), (40, 26), (60, 44)]
        return Art(
            shade: { ctx in for r in rows { ctx.fill(circle(r.x + 2, r.y + 2, 8), gray: 150) } },
            pink: { ctx in for r in rows { ctx.fill(circle(r.x, r.y, 8), gray: 40) } },
            ink: { ctx in
                ctx.pen(4.4)
                for r in rows {
                    ctx.stroke(line(10, r.y, r.x - 8, r.y)); ctx.stroke(line(r.x + 8, r.y, 70, r.y))
                    ctx.stroke(circle(r.x, r.y, 8))
                }
            })
    }()

    /// Shortcuts: a keycap with the command mark on its top, printed pink.
    private static let commandKey = Art(
        shade: { $0.fill(rounded(14, 18, 56, 52, 9), gray: 150) },
        pink: { $0.fill(rounded(18, 18, 44, 36, 6), gray: 60) },
        ink: { ctx in
            ctx.pen()
            ctx.stroke(rounded(12, 14, 56, 52, 9))
            // ⌘: a square with a loop at each corner.
            ctx.pen(3.6)
            let c = CGPoint(x: 40, y: 37), h: CGFloat = 6, r: CGFloat = 5
            ctx.stroke(line(c.x - h, c.y - h, c.x + h, c.y - h)); ctx.stroke(line(c.x - h, c.y + h, c.x + h, c.y + h))
            ctx.stroke(line(c.x - h, c.y - h, c.x - h, c.y + h)); ctx.stroke(line(c.x + h, c.y - h, c.x + h, c.y + h))
            for (dx, dy) in [(-1.0, -1.0), (1, -1), (-1, 1), (1, 1)] as [(CGFloat, CGFloat)] {
                ctx.stroke(circle(c.x + dx * (h + r), c.y + dy * (h + r), r))
            }
        })

    /// Tones: a speech bubble with a voice running through it.
    private static let bubble: Art = {
        func shape() -> CGPath {
            let p = CGMutablePath()
            p.addPath(rounded(10, 14, 60, 40, 12))
            p.move(to: CGPoint(x: 24, y: 52)); p.addLine(to: CGPoint(x: 17, y: 68)); p.addLine(to: CGPoint(x: 38, y: 53))
            return p
        }
        return Art(
            shade: { $0.fill(shape(), gray: 150) },
            pink: { $0.fill(shape(), gray: 75) },
            ink: { ctx in
                ctx.pen()
                ctx.stroke(rounded(10, 14, 60, 40, 12))
                let tail = CGMutablePath()
                tail.move(to: CGPoint(x: 24, y: 54)); tail.addLine(to: CGPoint(x: 17, y: 68)); tail.addLine(to: CGPoint(x: 38, y: 54))
                ctx.stroke(tail)
                ctx.pen(4.2)
                let wave = CGMutablePath()
                wave.move(to: CGPoint(x: 20, y: 34))
                for x in stride(from: 20.0, through: 60.0, by: 1.0) {
                    wave.addLine(to: CGPoint(x: x, y: 34 + 6 * sin((x - 20) / 40 * 2 * .pi)))
                }
                ctx.stroke(wave)
            })
    }()

    /// Words: an open book, the left page in pink, lines of text on both.
    private static let book: Art = {
        func page(_ mirror: Bool) -> CGPath {
            let f: (CGFloat) -> CGFloat = { mirror ? 80 - $0 : $0 }
            let p = CGMutablePath()
            p.move(to: CGPoint(x: 40, y: 26))
            p.addQuadCurve(to: CGPoint(x: f(9), y: 22), control: CGPoint(x: f(26), y: 15))
            p.addLine(to: CGPoint(x: f(9), y: 62))
            p.addQuadCurve(to: CGPoint(x: 40, y: 66), control: CGPoint(x: f(26), y: 55))
            p.closeSubpath()
            return p
        }
        return Art(
            shade: { $0.fill(CGPath(ellipseIn: CGRect(x: 12, y: 62, width: 60, height: 11), transform: nil), gray: 140) },
            pink: { ctx in ctx.fill(page(false), gray: 60); ctx.fill(page(true), gray: 175) },
            ink: { ctx in
                ctx.pen()
                ctx.stroke(page(false)); ctx.stroke(page(true))
                ctx.pen(3.6)
                for y: CGFloat in [36, 48] {
                    ctx.stroke(line(16, y, 33, y + 1.5))
                    ctx.stroke(line(64, y, 47, y + 1.5))
                }
            })
    }()

    /// Apps: four app tiles, one printed solid pink.
    private static let tiles: Art = {
        let origins: [(CGFloat, CGFloat)] = [(12, 12), (44, 12), (12, 44), (44, 44)]
        return Art(
            shade: { ctx in for (x, y) in origins { ctx.fill(rounded(x + 1, y + 1, 24, 24, 6), gray: 150) } },
            pink: { ctx in
                for (i, (x, y)) in origins.enumerated() { ctx.fill(rounded(x, y, 24, 24, 6), gray: i == 1 ? 25 : 180) }
            },
            ink: { ctx in
                ctx.pen()
                for (x, y) in origins { ctx.stroke(rounded(x, y, 24, 24, 6)) }
            })
    }()

    /// Model & memory: a chip with pins, its core in pink.
    private static let chip = Art(
        shade: { $0.fill(rounded(21, 22, 40, 40, 6), gray: 150) },
        pink: { ctx in ctx.fill(rounded(20, 20, 40, 40, 6), gray: 170); ctx.fill(rounded(30, 30, 20, 20, 3), gray: 20) },
        ink: { ctx in
            ctx.pen()
            ctx.stroke(rounded(20, 20, 40, 40, 6))
            ctx.pen(3.6)
            ctx.stroke(rounded(30, 30, 20, 20, 3))
            ctx.pen(4)
            for i in 0..<3 {
                let c = 29 + CGFloat(i) * 11
                ctx.stroke(line(c, 11, c, 19)); ctx.stroke(line(c, 61, c, 69))
                ctx.stroke(line(11, c, 19, c)); ctx.stroke(line(61, c, 69, c))
            }
        })

    /// Privacy: a padlock, shut.
    private static let lock = Art(
        shade: { $0.fill(rounded(18, 37, 48, 36, 7), gray: 150) },
        pink: { $0.fill(rounded(16, 34, 48, 36, 7), gray: 60) },
        ink: { ctx in
            ctx.pen(5.4)
            let shackle = CGMutablePath()
            shackle.move(to: CGPoint(x: 27, y: 34)); shackle.addLine(to: CGPoint(x: 27, y: 25))
            shackle.addCurve(to: CGPoint(x: 53, y: 25), control1: CGPoint(x: 27, y: 8), control2: CGPoint(x: 53, y: 8))
            shackle.addLine(to: CGPoint(x: 53, y: 34))
            ctx.stroke(shackle)
            ctx.pen()
            ctx.stroke(rounded(16, 34, 48, 36, 7))
            ctx.fill(circle(40, 49, 5), gray: 0)
            ctx.pen(4.4)
            ctx.stroke(line(40, 51, 40, 60))
        })

    /// Updates (and Check for Updates in the menu): a sprout coming up, its new leaf pink, the
    /// way the garden grows while a new Iris builds.
    private static let sprout: Art = {
        func leaf(_ base: CGPoint, _ tip: CGPoint, bulge: CGFloat) -> CGPath {
            let p = CGMutablePath()
            let mid = CGPoint(x: (base.x + tip.x) / 2, y: (base.y + tip.y) / 2)
            let dx = tip.x - base.x, dy = tip.y - base.y
            let n = CGPoint(x: -dy / hypot(dx, dy) * bulge, y: dx / hypot(dx, dy) * bulge)
            p.move(to: base)
            p.addQuadCurve(to: tip, control: CGPoint(x: mid.x + n.x, y: mid.y + n.y))
            p.addQuadCurve(to: base, control: CGPoint(x: mid.x - n.x, y: mid.y - n.y))
            return p
        }
        let old = leaf(CGPoint(x: 39, y: 46), CGPoint(x: 13, y: 34), bulge: 10)
        let new = leaf(CGPoint(x: 41, y: 34), CGPoint(x: 66, y: 14), bulge: 12)
        func stem() -> CGPath {
            let p = CGMutablePath()
            p.move(to: CGPoint(x: 40, y: 66))
            p.addCurve(to: CGPoint(x: 41, y: 32), control1: CGPoint(x: 36, y: 54), control2: CGPoint(x: 44, y: 44))
            return p
        }
        return Art(
            shade: { ctx in
                ctx.fill(CGPath(ellipseIn: CGRect(x: 14, y: 62, width: 54, height: 11), transform: nil), gray: 140)
                var nudge = CGAffineTransform(translationX: 2, y: 2)
                ctx.fill(new.copy(using: &nudge)!, gray: 150)
            },
            pink: { ctx in ctx.fill(new, gray: 40); ctx.fill(old, gray: 170) },
            ink: { ctx in
                ctx.pen()
                ctx.stroke(stem()); ctx.stroke(old); ctx.stroke(new)
                ctx.stroke(line(14, 67, 66, 66))
                ctx.pen(3.4)
                ctx.stroke(line(41, 34, 58, 21))
            })
    }()

    /// About: a pen nib, the hand that made Iris. One half printed pink.
    private static let nib: Art = {
        func outline() -> CGPath {
            let p = CGMutablePath()
            p.move(to: CGPoint(x: 40, y: 72))
            p.addCurve(to: CGPoint(x: 22, y: 30), control1: CGPoint(x: 32, y: 58), control2: CGPoint(x: 20, y: 44))
            p.addLine(to: CGPoint(x: 28, y: 12))
            p.addLine(to: CGPoint(x: 52, y: 12))
            p.addLine(to: CGPoint(x: 58, y: 30))
            p.addCurve(to: CGPoint(x: 40, y: 72), control1: CGPoint(x: 60, y: 44), control2: CGPoint(x: 48, y: 58))
            p.closeSubpath()
            return p
        }
        func half() -> CGPath {
            let p = CGMutablePath()
            p.move(to: CGPoint(x: 40, y: 72))
            p.addCurve(to: CGPoint(x: 22, y: 30), control1: CGPoint(x: 32, y: 58), control2: CGPoint(x: 20, y: 44))
            p.addLine(to: CGPoint(x: 28, y: 12))
            p.addLine(to: CGPoint(x: 40, y: 12))
            p.closeSubpath()
            return p
        }
        return Art(
            shade: { ctx in
                var nudge = CGAffineTransform(translationX: 2, y: 2)
                ctx.fill(outline().copy(using: &nudge)!, gray: 150)
            },
            pink: { ctx in ctx.fill(outline(), gray: 175); ctx.fill(half(), gray: 45) },
            ink: { ctx in
                ctx.pen()
                ctx.stroke(outline())
                ctx.stroke(line(26, 24, 54, 24))
                ctx.fill(circle(40, 40, 4.5), gray: 0)
                ctx.pen(3.4)
                ctx.stroke(line(40, 44, 40, 70))
            })
    }()

    /// The clock for minutes: an alarm clock with two pink bells and little feet.
    private static let alarmClock = Art(
        shade: { ctx in ctx.fill(circle(41, 47, 25), gray: 150) },
        pink: { ctx in
            ctx.fill(circle(40, 45, 24), gray: 120)
            ctx.fill(CGPath(ellipseIn: CGRect(x: 10, y: 10, width: 18, height: 15), transform: nil), gray: 30)
            ctx.fill(CGPath(ellipseIn: CGRect(x: 52, y: 10, width: 18, height: 15), transform: nil), gray: 30)
        },
        ink: { ctx in
            ctx.pen()
            ctx.stroke(CGPath(ellipseIn: CGRect(x: 10, y: 10, width: 18, height: 15), transform: nil))
            ctx.stroke(CGPath(ellipseIn: CGRect(x: 52, y: 10, width: 18, height: 15), transform: nil))
            ctx.fill(circle(40, 45, 24), gray: 255)  // the face sits over the bells
            ctx.stroke(circle(40, 45, 24))
            ctx.stroke(line(27, 65, 21, 73)); ctx.stroke(line(53, 65, 59, 73))
            ctx.stroke(line(40, 45, 40, 30)); ctx.stroke(line(40, 45, 51, 51))
            ctx.fill(circle(40, 45, 4), gray: 0)
        })

    /// Quick Fix: a magic wand, its star pink, a few sparks flying off. One press and it's fixed.
    private static let magicWand: Art = {
        let star: CGPath = {
            let p = CGMutablePath()
            for i in 0..<10 {
                let a = -CGFloat.pi / 2 + CGFloat(i) * .pi / 5
                let r: CGFloat = i % 2 == 0 ? 15 : 6.75
                let pt = CGPoint(x: 56 + r * cos(a), y: 22 + r * sin(a))
                i == 0 ? p.move(to: pt) : p.addLine(to: pt)
            }
            p.closeSubpath()
            return p
        }()
        return Art(
            shade: { ctx in
                var nudge = CGAffineTransform(translationX: 2, y: 2)
                ctx.fill(star.copy(using: &nudge)!, gray: 150)
            },
            pink: { $0.fill(star, gray: 30) },
            ink: { ctx in
                ctx.pen(4); ctx.stroke(star)
                ctx.pen(6.4); ctx.stroke(line(14, 68, 46, 34))
                ctx.pen(3.6)
                ctx.stroke(line(22, 22, 26, 30)); ctx.stroke(line(14, 36, 22, 38)); ctx.stroke(line(62, 50, 66, 58))
            })
    }()

    /// Reads only when you ask: a raised hand.
    private static let palm: Art = {
        func shape() -> CGPath {
            var hand: CGPath = rounded(22, 38, 36, 30, 11)
            for (x, top) in [(23.0, 22.0), (32.0, 15.0), (41.0, 17.0), (50.0, 23.0)] as [(CGFloat, CGFloat)] {
                hand = hand.union(rounded(x, top, 8.5, 50 - top, 4.25))
            }
            var turn = CGAffineTransform(translationX: 16, y: 40).rotated(by: -0.6)
            let thumb = rounded(0, 0, 9, 22, 4.5).copy(using: &turn)!
            return hand.union(thumb)
        }
        return Art(
            shade: { ctx in
                var nudge = CGAffineTransform(translationX: 1, y: 2)
                ctx.fill(shape().copy(using: &nudge)!, gray: 150)
            },
            pink: { $0.fill(shape(), gray: 85) },
            ink: { ctx in ctx.pen(4.2); ctx.stroke(shape()) })
    }()

    /// Never sends for you: a paper plane, its near wing pink.
    private static let plane: Art = {
        func outline() -> CGPath {
            let p = CGMutablePath()
            p.addLines(between: [CGPoint(x: 68, y: 14), CGPoint(x: 10, y: 38), CGPoint(x: 32, y: 45),
                                 CGPoint(x: 40, y: 64), CGPoint(x: 68, y: 14)])
            p.closeSubpath()
            return p
        }
        func nearWing() -> CGPath {
            let p = CGMutablePath()
            p.addLines(between: [CGPoint(x: 68, y: 14), CGPoint(x: 32, y: 45), CGPoint(x: 40, y: 64)])
            p.closeSubpath()
            return p
        }
        return Art(
            shade: { $0.fill(CGPath(ellipseIn: CGRect(x: 14, y: 66, width: 40, height: 8), transform: nil), gray: 140) },
            pink: { ctx in ctx.fill(outline(), gray: 170); ctx.fill(nearWing(), gray: 30) },
            ink: { ctx in
                ctx.pen()
                ctx.stroke(outline())
                ctx.pen(3.6)
                ctx.stroke(line(68, 14, 32, 45))
                ctx.stroke(line(8, 56, 18, 52)); ctx.stroke(line(14, 64, 24, 60))
            })
    }()

    /// Keeps no history: an empty tray.
    private static let emptyTray: Art = {
        func body() -> CGPath {
            let p = CGMutablePath()
            p.addLines(between: [CGPoint(x: 10, y: 42), CGPoint(x: 22, y: 20), CGPoint(x: 58, y: 20),
                                 CGPoint(x: 70, y: 42), CGPoint(x: 70, y: 64), CGPoint(x: 10, y: 64)])
            p.closeSubpath()
            return p
        }
        func front() -> CGPath {
            let p = CGMutablePath()
            p.addLines(between: [CGPoint(x: 10, y: 42), CGPoint(x: 27, y: 42), CGPoint(x: 31, y: 50),
                                 CGPoint(x: 49, y: 50), CGPoint(x: 53, y: 42), CGPoint(x: 70, y: 42),
                                 CGPoint(x: 70, y: 64), CGPoint(x: 10, y: 64)])
            p.closeSubpath()
            return p
        }
        return Art(
            shade: { $0.fill(rounded(12, 24, 60, 44, 4), gray: 150) },
            pink: { ctx in ctx.fill(body(), gray: 185); ctx.fill(front(), gray: 60) },
            ink: { ctx in ctx.pen(); ctx.stroke(body()); ctx.stroke(front()) })
    }()

    /// Report a bug: a ladybird from the garden, seen from above, its shell pink with ink spots.
    private static let ladybird: Art = {
        let shell = CGPath(ellipseIn: CGRect(x: 19, y: 26, width: 42, height: 44), transform: nil)
        return Art(
            shade: { ctx in
                var nudge = CGAffineTransform(translationX: 2, y: 3)
                ctx.fill(shell.copy(using: &nudge)!, gray: 150)
            },
            pink: { $0.fill(shell, gray: 35) },
            ink: { ctx in
                ctx.pen(3.8)
                // Legs and feelers first, so the shell's line covers where they meet it.
                let legs: [(CGFloat, CGFloat, CGFloat, CGFloat)] = [(22, 38, 11, 33), (20, 49, 9, 50), (22, 60, 12, 67)]
                for (x1, y1, x2, y2) in legs {
                    ctx.stroke(line(x1, y1, x2, y2)); ctx.stroke(line(80 - x1, y1, 80 - x2, y2))
                }
                ctx.stroke(line(35, 17, 29, 8)); ctx.stroke(line(45, 17, 51, 8))
                ctx.fill(circle(40, 24, 9), gray: 25)
                ctx.pen()
                ctx.stroke(shell)
                ctx.pen(3.6)
                ctx.stroke(line(40, 30, 40, 69))
                let spots: [(CGFloat, CGFloat, CGFloat)] = [(30, 41, 4.6), (50, 41, 4.6), (29, 57, 3.8), (51, 57, 3.8)]
                for (x, y, r) in spots { ctx.fill(circle(x, y, r), gray: 25) }
            })
    }()

    /// About's note, a monthly fee: a price tag on its string, pink, with the price as two lines.
    private static let tag: Art = {
        func shape() -> CGPath {
            let p = CGMutablePath()
            p.addLines(between: [CGPoint(x: 12, y: 40), CGPoint(x: 28, y: 20), CGPoint(x: 68, y: 20),
                                 CGPoint(x: 68, y: 60), CGPoint(x: 28, y: 60)])
            p.closeSubpath()
            return p
        }
        return Art(
            shade: { ctx in
                var nudge = CGAffineTransform(translationX: 2, y: 3)
                ctx.fill(shape().copy(using: &nudge)!, gray: 150)
            },
            pink: { $0.fill(shape(), gray: 45) },
            ink: { ctx in
                ctx.pen()
                ctx.stroke(shape())
                ctx.pen(3.6)
                ctx.stroke(circle(28, 40, 4.5))
                ctx.stroke(line(42, 33, 60, 33)); ctx.stroke(line(42, 46, 55, 46))
                // The string, looping off the hole.
                let string = CGMutablePath()
                string.move(to: CGPoint(x: 24, y: 40))
                string.addCurve(to: CGPoint(x: 6, y: 12), control1: CGPoint(x: 10, y: 40), control2: CGPoint(x: 2, y: 24))
                ctx.stroke(string)
            })
    }()

    /// About's note, someone else's server: a cloud with an eye in it, reading.
    private static let cloudEye: Art = {
        func cloud() -> CGPath {
            var c: CGPath = rounded(12, 40, 58, 22, 11)
            for (x, y, r) in [(28.0, 40.0, 13.0), (44.0, 32.0, 17.0), (58.0, 42.0, 12.0)] as [(CGFloat, CGFloat, CGFloat)] {
                c = c.union(circle(x, y, r))
            }
            return c
        }
        func eye() -> CGPath {
            let p = CGMutablePath()
            p.move(to: CGPoint(x: 28, y: 44))
            p.addQuadCurve(to: CGPoint(x: 54, y: 44), control: CGPoint(x: 41, y: 32))
            p.addQuadCurve(to: CGPoint(x: 28, y: 44), control: CGPoint(x: 41, y: 56))
            p.closeSubpath()
            return p
        }
        return Art(
            shade: { ctx in
                var nudge = CGAffineTransform(translationX: 2, y: 3)
                ctx.fill(cloud().copy(using: &nudge)!, gray: 150)
            },
            pink: { ctx in ctx.fill(cloud(), gray: 170); ctx.fill(eye(), gray: 40) },
            ink: { ctx in
                ctx.pen(); ctx.stroke(cloud())
                ctx.pen(3.4); ctx.stroke(eye())
                ctx.fill(circle(41, 44, 4.2), gray: 25)
            })
    }()

    /// About's note, help that stops offline: the Wi-Fi fan, its middle band pink.
    private static let signal: Art = {
        let c = CGPoint(x: 40, y: 62)
        func arc(_ r: CGFloat) -> CGPath {
            let p = CGMutablePath()
            p.addArc(center: c, radius: r, startAngle: -.pi * 0.78, endAngle: -.pi * 0.22, clockwise: false)
            return p
        }
        func band() -> CGPath {
            let p = CGMutablePath()
            p.addArc(center: c, radius: 28, startAngle: -.pi * 0.78, endAngle: -.pi * 0.22, clockwise: false)
            p.addArc(center: c, radius: 16, startAngle: -.pi * 0.22, endAngle: -.pi * 0.78, clockwise: true)
            p.closeSubpath()
            return p
        }
        return Art(
            shade: { ctx in
                var nudge = CGAffineTransform(translationX: 2, y: 3)
                ctx.fill(band().copy(using: &nudge)!, gray: 150)
            },
            pink: { $0.fill(band(), gray: 45) },
            ink: { ctx in
                ctx.pen()
                for r in [16.0, 28.0, 40.0] as [CGFloat] { ctx.stroke(arc(r)) }
                ctx.fill(circle(c.x, c.y, 5), gray: 25)
            })
    }()

    // MARK: Helpers

    private static func circle(_ x: CGFloat, _ y: CGFloat, _ r: CGFloat) -> CGPath {
        CGPath(ellipseIn: CGRect(x: x - r, y: y - r, width: r * 2, height: r * 2), transform: nil)
    }
    private static func rounded(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, _ r: CGFloat) -> CGPath {
        CGPath(roundedRect: CGRect(x: x, y: y, width: w, height: h), cornerWidth: r, cornerHeight: r, transform: nil)
    }
    private static func line(_ x1: CGFloat, _ y1: CGFloat, _ x2: CGFloat, _ y2: CGFloat) -> CGPath {
        let p = CGMutablePath()
        p.move(to: CGPoint(x: x1, y: y1)); p.addLine(to: CGPoint(x: x2, y: y2))
        return p
    }
}

private extension CGContext {
    /// The ink pen: near-black on the plate, so the espresso prints almost solid.
    func pen(_ width: CGFloat = 4.8) {
        setStrokeColor(gray: 25 / 255, alpha: 1)
        setLineWidth(width)
        setLineCap(.round)
        setLineJoin(.round)
    }
    func stroke(_ path: CGPath) { addPath(path); strokePath() }
    func fill(_ path: CGPath, gray: CGFloat) { addPath(path); setFillColor(gray: gray / 255, alpha: 1); fillPath() }
}

/// A riso icon at a given point size, kept pixel-crisp.
struct RisoIconView: View {
    let icon: RisoIcon
    var size: CGFloat = 24
    var body: some View {
        Image(nsImage: icon.image()).interpolation(.none).resizable()
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}
