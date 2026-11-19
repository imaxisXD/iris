import IrisEngine
import SwiftUI

/// One model to choose, drawn like a brewer on the zine's page: the chip with its pink off
/// register, ringed in pink brush when it's the chosen one, its name and its size stacked.
/// Settings › Model & memory draws it big with a line on what it's for, and can cross it out
/// (Delete under the pointer); setup draws it smaller. One view, so what the two say, and what
/// VoiceOver hears, can't drift apart.
struct ModelOption: View {
    enum Place { case settings, setup }

    let choice: ModelChoice
    /// Its place in the row: the first is drawn bigger and tilted the other way.
    let index: Int
    let place: Place
    let chosen: Bool
    let onDisk: Bool
    /// Delete is under the pointer: a dry-brush X lands on the drawing.
    var crossed = false
    let choose: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let big = place == .settings
        let first = index == 0
        let figure = choice.downloadFigure
        Button(action: choose) {
            VStack(alignment: .leading, spacing: big ? 10 : 8) {
                ZStack {
                    RisoDrawing(icon: .model, size: big ? (first ? 92 : 74) : (first ? 74 : 60))
                        .rotationEffect(.degrees(first ? -6 : 5))
                    if chosen {
                        BrushRing(rgb: Riso.pink, seed: 13 + index)
                            .frame(width: big ? 124 : 98, height: big ? 112 : 86)
                    }
                    if crossed {
                        DryBrushX(seed: 4 + index, dryness: 0.65)
                            .frame(width: first ? 132 : 116, height: first ? 114 : 100)
                            .transition(reduceMotion ? .opacity : .scale(scale: 0.8).combined(with: .opacity))
                    }
                }
                .frame(width: big ? 130 : 104, height: big ? 116 : 88)
                .animation(DS.easeOut, value: chosen)
                .animation(DS.easeOut, value: crossed)
                if big {
                    InkLabel(text: choice.plainName, size: 12)
                } else {
                    Text(choice.plainName).font(DS.Font.body).foregroundStyle(DS.Ink.i12)
                }
                StackedFigure(value: figure.value, unit: figure.unit, note: onDisk ? "on disk" : "to download")
                if big {
                    Text(choice.shortLine).font(DS.Font.caption).foregroundStyle(DS.Ink.i10)
                        .fixedSize(horizontal: false, vertical: true).frame(width: 210, alignment: .leading)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(SqueezeStyle(scale: 0.99))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(choice.plainName), \(choice.downloadSize). \(choice.shortLine)")
        // `RecommendedMark` beside it is a picture: the button says it.
        .accessibilityValue(choice == .recommended ? "Recommended for this Mac" : "")
        .accessibilityAddTraits(chosen ? [.isButton, .isSelected] : .isButton)
    }
}

/// "Use this one", under a model that isn't the chosen one.
struct UseModelButton: View {
    let choice: ModelChoice
    let choose: () -> Void

    var body: some View {
        TactileButton(title: "Use this one", role: .secondary, compact: true, action: choose)
            .fixedSize()
            .accessibilityLabel("Use \(choice.plainName.lowercased())")
    }
}
