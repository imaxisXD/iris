import AppKit
import IrisCore
import SwiftUI

/// Design check: `Iris --rig-frames <dir>` renders the ⌃G moment off screen, frame by frame at
/// exact times (25 fps), over a stand-in chat: the flower's arrival, the bloom, the panel
/// unrolling from it, and the close after Replace. Plays the full arrival, then the quick one.
/// Nothing is shown on screen. The panel's unroll copies PanelView's spring by hand.
@MainActor
enum RigFrames {
    static let scene = CGSize(width: 900, height: 460)
    static let box = CGRect(x: 200, y: 330, width: 560, height: 58)

    struct Script { let name: String; let full: Bool; let bloom: Double; let replace: Double; let end: Double }

    static func run(into dir: String) {
        FontRegistry.registerAll()
        let original = OnboardingModel.practiceSample
        let fixed = OnboardingModel.practiceFixed
        let model = PanelModel()
        model.preview(original: original,
                      result: RewriteResult(mode: .fix, original: original, text: fixed, segments: WordDiff.diff(original, fixed),
                                            verdict: .minor, latencyMs: 900),
                      engine: .ready(.best))
        let renderer = ImageRenderer(content: PanelView(model: model).environment(\.colorScheme, .light))
        renderer.scale = 2
        guard let panel = renderer.nsImage else { return }
        let scripts = [Script(name: "full", full: true, bloom: 0.9, replace: 3.0, end: 4.5),
                       Script(name: "quick", full: false, bloom: 0.5, replace: 2.2, end: 3.7)]
        for script in scripts {
            let folder = "\(dir)/\(script.name)"
            try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
            var frame = 0
            var t = -0.12
            while t <= script.end {
                ComponentSnapshot.renderPure(Frame(t: t, script: script, panel: panel, original: original, fixed: fixed),
                                             width: scene.width, to: String(format: "%@/%03d.png", folder, frame))
                frame += 1
                t += 0.04
            }
        }
    }

    /// One moment of the scene.
    struct Frame: View {
        let t: Double
        let script: Script
        let panel: NSImage
        let original: String
        let fixed: String

        var body: some View {
            // Placed as PanelController does, flipped to this scene's top-left origin.
            let m = PanelView.margin
            let size = panel.size
            let placed = PanelLayout.place(paper: CGSize(width: size.width - m.horizontal * 2, height: size.height - m.top - m.bottom),
                                           by: CGRect(x: box.minX, y: scene.height - box.maxY, width: box.width, height: box.height),
                                           prefer: .above, in: CGRect(origin: .zero, size: scene), flower: BudView.reach)
            let foot = CGPoint(x: placed.foot.x, y: scene.height - placed.foot.y)
            let clock = RigClock(now: 1000 + t, arrived: t >= 0 ? 1000 : nil,
                                 bloomed: t >= script.bloom ? 1000 + script.bloom : nil,
                                 wilted: t >= script.replace ? 1000 + script.replace : nil, full: script.full)
            // The panel unrolls when PanelController would: the answer is in as the bud blooms.
            let unrollAt = script.bloom + PanelMotion.unrollDelay(waited: script.bloom, bloomed: true, full: script.full)
            let u = t >= script.replace ? -1
                : Curve.spring(t - unrollAt, PanelMotion.unroll.duration, bounce: PanelMotion.unroll.bounce)
            let fade = Curve.easeOut((t - unrollAt) / PanelMotion.unrollFade)
            let origin = CGPoint(x: placed.paper.minX - m.horizontal, y: scene.height - placed.paper.maxY - m.top)
            let hinge = CGPoint(x: placed.hinge.x, y: scene.height - placed.hinge.y)
            let anchor = UnitPoint(x: (hinge.x - origin.x) / size.width, y: (hinge.y - origin.y) / size.height)
            ZStack(alignment: .topLeading) {
                Color(red: 0.965, green: 0.962, blue: 0.97)
                // The chat: a message, then the composer box.
                HStack(alignment: .top, spacing: 10) {
                    RoundedRectangle(cornerRadius: 6).fill(Color(red: 0.79, green: 0.72, blue: 0.85)).frame(width: 30, height: 30)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("lead").font(.system(size: 13, weight: .bold))
                        Text("Did you get to the PR?").font(.system(size: 13))
                    }
                }
                .foregroundStyle(.black)
                .offset(x: box.minX + 40, y: box.minY - 62)
                Text(t >= script.replace + 0.05 ? fixed : original).font(.system(size: 13)).foregroundStyle(.black)
                    .frame(width: box.width - 24, height: box.height - 20, alignment: .topLeading)
                    .padding(.horizontal, 12).padding(.vertical, 10)
                    .background(RoundedRectangle(cornerRadius: 8).fill(.white))
                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.gray.opacity(0.5)))
                    .offset(x: box.minX, y: box.minY)
                if u > 0 {
                    Image(nsImage: panel).resizable().frame(width: size.width, height: size.height)
                        .scaleEffect(x: PanelMotion.folded.scaleX + (1 - PanelMotion.folded.scaleX) * u,
                                     y: PanelMotion.folded.scaleY + (1 - PanelMotion.folded.scaleY) * u, anchor: anchor)
                        .rotationEffect(.degrees(PanelMotion.folded.degrees * (1 - u)), anchor: anchor)
                        .opacity(fade)
                        .offset(x: origin.x, y: origin.y)
                }
                BudScene(clock: clock, size: BudView.size)
                    .frame(width: BudView.box.width, height: BudView.box.height - BudView.foot, alignment: .bottom)
                    .offset(x: foot.x - BudView.box.width / 2, y: foot.y - (BudView.box.height - BudView.foot))
                    .opacity(t >= 0 ? 1 : 0)
            }
            .frame(width: scene.width, height: scene.height, alignment: .topLeading)
            .clipped()
        }
    }
}
