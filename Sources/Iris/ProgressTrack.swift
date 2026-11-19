import SwiftUI

/// Progress as a swipe of pink marker across a faint pen line, the share in brush at its
/// tip, like the menu's row highlight. Never a capsule in a well. `nil`: under way, amount
/// unknown (a short, faint swipe and no number).
struct ProgressTrack: View {
    let fraction: Double?

    var body: some View {
        GeometryReader { geo in
            let f = min(1, max(0, fraction ?? 0.3))
            let room: CGFloat = fraction == nil ? 0 : 44  // for the brush number at the tip
            let w = max(16, (geo.size.width - room) * f)
            ZStack(alignment: .leading) {
                PenRule(seed: 91, ink: DS.Ink.i12.opacity(0.18)).frame(maxHeight: .infinity)
                // The pink plate printed through the screen, at a marker's strength.
                HandRect(seed: 92, wobble: 0.7, closed: true).printedInk(Riso.pink, seed: 92)
                    .opacity(0.45)
                    .frame(width: w, height: 12)
                    .opacity(fraction == nil ? 0.6 : 1)
                if let fraction {
                    Text("\(Int(fraction * 100))%").font(DS.Font.brush(17)).foregroundStyle(DS.Hue.pinkText)
                        .monospacedDigit().fixedSize()
                        .offset(x: w + 6)
                }
            }
        }
        .frame(height: 18)
        .accessibilityElement()
        .accessibilityLabel("Progress")
        .accessibilityValue(fraction.map { "\(Int($0 * 100)) percent" } ?? "Under way")
    }
}
