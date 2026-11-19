import IrisEngine
import SwiftUI

/// What each model is good at, measured: blind-judged answers on drafts no lesson was written
/// from (the share a person would accept or send), answer time and memory on an Apple
/// silicon Mac. Shown as dots under each model in setup and Settings › Model & memory.
struct ModelStrengths: Equatable {
    /// Share of English answers good enough to send (Fix and the tones), out of 100.
    let english: Int
    /// The same for Hindi and Hinglish drafts.
    let hindi: Int
    /// Seconds for a typical answer.
    let seconds: Double
    /// Memory it holds while loaded, in GB.
    let memoryGB: Double

    static func of(_ choice: ModelChoice) -> ModelStrengths? {
        switch choice {
        case .best: ModelStrengths(english: 91, hindi: 85, seconds: 0.3, memoryGB: 3.7)
        case .lighter: ModelStrengths(english: 87, hindi: 59, seconds: 0.2, memoryGB: 2.9)
        // Judged in the same rounds as Iris's own; memory as measured with Iris before.
        case .e4b: ModelStrengths(english: 71, hindi: 81, seconds: 0.45, memoryGB: 5.2)
        case .e2b: nil
        }
    }

    /// Dots out of five, to the half: a share out of 100, or how quick an answer is.
    static func dots(share: Int) -> Double { (Double(share) / 20 * 2).rounded() / 2 }
    static func dots(seconds: Double) -> Double { seconds <= 0.2 ? 5 : seconds <= 0.3 ? 4 : seconds <= 0.5 ? 3 : 2 }
}

/// Why the recommended model suits this Mac, in its own words.
func recommendationLine(memoryGB: Int) -> String {
    ModelChoice.recommended(memoryGB: memoryGB) == .best
        ? "This Mac has \(memoryGB) GB of memory, room for best quality."
        : "This Mac has \(memoryGB) GB of memory. Lighter leaves room for your other apps."
}

/// One model's strengths as printed dots out of five, like a recipe card: English, Hindi,
/// speed, then the memory it takes as a figure. Under the model's drawing.
struct ModelPips: View {
    let strengths: ModelStrengths
    /// Varies the dots' wobble from model to model.
    var seed = 0

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            row("English", ModelStrengths.dots(share: strengths.english), seed: seed)
            row("Hindi", ModelStrengths.dots(share: strengths.hindi), seed: seed + 5)
            row("Speed", ModelStrengths.dots(seconds: strengths.seconds), seed: seed + 9)
            HStack(spacing: 6) {
                Text("Memory").font(DS.Font.caption).foregroundStyle(DS.Ink.i10).frame(width: 64, alignment: .leading)
                Text(String(format: "%.1f GB", strengths.memoryGB)).font(DS.Font.caption).foregroundStyle(DS.Ink.i12)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(String(format: "English: %d of 100 answers good enough to send. Hindi: %d of 100. An answer in about %.1f seconds. Takes %.1f GB of memory.",
                                   strengths.english, strengths.hindi, strengths.seconds, strengths.memoryGB))
    }

    private func row(_ label: String, _ score: Double, seed: Int) -> some View {
        HStack(spacing: 6) {
            Text(label).font(DS.Font.caption).foregroundStyle(DS.Ink.i10).frame(width: 64, alignment: .leading)
            HStack(spacing: 4) {
                ForEach(0..<5) { k in
                    Pip(fill: min(1, max(0, score - Double(k))), seed: seed * 7 + k)
                        .offset(x: CGFloat(k % 2) * 0.4, y: CGFloat((k + seed) % 3) * 0.4 - 0.4)
                }
            }
        }
    }
}

/// One dot: a pen circle, filled with the pink plate to `fill` (0, half or whole).
private struct Pip: View {
    let fill: Double
    let seed: Int

    var body: some View {
        ZStack(alignment: .leading) {
            Circle().strokeBorder(DS.Ink.i12.opacity(0.75), lineWidth: 1.2)
            if fill > 0 {
                Circle().printedInk(Riso.pink, seed: seed)
                    .mask(Rectangle().frame(width: 12 * fill).frame(maxWidth: .infinity, alignment: .leading))
            }
        }
        .frame(width: 12, height: 12)
    }
}

/// The note under the models: why one is recommended, and what the dots measure.
struct ModelPipsNote: View {
    var body: some View {
        (Text(recommendationLine(memoryGB: ModelChoice.macMemoryGB)).foregroundStyle(DS.Ink.i11)
            + Text(" Dots: how often an answer was good enough to send, in blind tests.").foregroundStyle(DS.Ink.i10))
            .font(DS.Font.caption).fixedSize(horizontal: false, vertical: true)
    }
}
