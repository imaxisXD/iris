import AppKit
import IrisCore
import IrisEngine
import SwiftUI

/// Design check: `Iris --panel-states <dir>` draws the real panel in every state its bottom row
/// can be in (Fix and a tone; working, answered, a big rewrite, a translation, nothing to fix,
/// failed; a short, long or missing app name; text that can only be copied; the copied and
/// try-again notes), four to a sheet, each captioned with what `PanelFooter` says should show.
/// Nothing is read, written or pasted.
@MainActor
enum PanelStates {
    struct State {
        let caption: String
        let setup: (PanelModel) -> Void
    }

    static func run(into dir: String) {
        FontRegistry.registerAll()
        let original = "hey, i have checked the PR and its look fine but one thing need to change before merge @lead"
        let fixed = "hey, I have checked the PR and it looks fine, but one thing needs to change before merge @lead"
        let rewrite = "Hi @lead, I've reviewed the PR. It looks good overall, but one change is needed before we merge."
        let friendly = "Hey @lead! I checked the PR and it looks good. Just one small thing to change before we merge it."
        let clean = "Can you review this when you get a chance? @lead"
        let hinglish = "bhai order kal tak aa jayega, customer ko bol do"
        let english = "The order will arrive by tomorrow, tell the customer."
        let tone = ToneList.newMac.shown[1]  // what `PanelModel.preview` shows second
        let long = (name: "T3 Code (Nightly) Preview Edition", id: "com.t3tools.t3code")

        func fix(_ from: String, _ to: String, _ verdict: FixGuard.Verdict.Kind?, translated: Bool = false) -> RewriteResult {
            RewriteResult(mode: .fix, original: from, text: to, segments: WordDiff.diff(from, to), verdict: verdict,
                          translated: translated, latencyMs: 300)
        }
        func toned(_ from: String, _ to: String, translated: Bool = false) -> RewriteResult {
            RewriteResult(mode: tone, original: from, text: to, segments: WordDiff.diff(from, to), verdict: nil,
                          translated: translated, latencyMs: 600)
        }
        let ready = EngineState.ready(.best)

        let states: [State] = [
            State(caption: "Fix · checking") { $0.preview(original: original, result: nil, engine: ready, streaming: "hey, I") },
            State(caption: "Fix · fixed · Slack") { $0.preview(original: original, result: fix(original, fixed, .minor), engine: ready) },
            State(caption: "Fix · fixed · long app name") {
                $0.preview(original: original, result: fix(original, fixed, .minor), engine: ready, appName: long.name, appBundleID: long.id)
            },
            State(caption: "Fix · fixed · app unknown") {
                $0.preview(original: original, result: fix(original, fixed, .minor), engine: ready, appName: nil, appBundleID: nil)
            },
            State(caption: "Fix · fixed · can only copy") {
                $0.preview(original: original, result: fix(original, fixed, .minor), engine: ready, canReplace: false)
            },
            State(caption: "Fix · big rewrite · Slack") { $0.preview(original: original, result: fix(original, rewrite, .heavy), engine: ready) },
            State(caption: "Fix · big rewrite · long app name") {
                $0.preview(original: original, result: fix(original, rewrite, .heavy), engine: ready, appName: long.name, appBundleID: long.id)
            },
            State(caption: "Fix · translated · long app name") {
                $0.preview(original: hinglish, result: fix(hinglish, english, nil, translated: true), engine: ready, appName: long.name, appBundleID: long.id)
            },
            State(caption: "Fix · nothing to fix") { $0.preview(original: clean, result: fix(clean, clean, .unchanged), engine: ready) },
            State(caption: "Fix · after ⌘R, came out the same") {
                $0.preview(original: original, result: fix(original, fixed, .minor), engine: ready, retryNote: "Came out the same")
            },
            State(caption: "Fix · can only copy · copied") {
                $0.preview(original: original, result: fix(original, fixed, .minor), engine: ready, canReplace: false, copied: true)
            },
            State(caption: "Fix · failed, can try again") {
                $0.preview(original: original, result: nil, engine: ready, error: RewriteError.timedOut)
            },
            State(caption: "Fix · waking the model") { $0.preview(original: original, result: nil, engine: .loading) },
            State(caption: "Fix · the model isn't on this Mac") {
                $0.preview(original: original, result: nil, engine: .idle, error: EngineError.notDownloaded)
            },
            State(caption: "Fix · failed, too long") {
                $0.preview(original: original, result: nil, engine: ready, error: RewriteError.tooLong(limit: Rewriter.maxInputCharacters))
            },
            State(caption: "\(tone.label) · writing") {
                $0.preview(original: original, result: nil, selected: 1, engine: ready, streaming: "Hey @lead!")
            },
            State(caption: "\(tone.label) · done · Slack") { $0.preview(original: original, result: toned(original, friendly), selected: 1, engine: ready) },
            State(caption: "\(tone.label) · done · long app name") {
                $0.preview(original: original, result: toned(original, friendly), selected: 1, engine: ready, appName: long.name, appBundleID: long.id)
            },
            State(caption: "\(tone.label) · done · can only copy") {
                $0.preview(original: original, result: toned(original, friendly), selected: 1, engine: ready, canReplace: false)
            },
            State(caption: "\(tone.label) · translated · Slack") {
                $0.preview(original: hinglish, result: toned(hinglish, english, translated: true), selected: 1, engine: ready)
            },
            State(caption: "\(tone.label) · after ⌘R, came out the same") {
                $0.preview(original: original, result: toned(original, friendly), selected: 1, engine: ready, retryNote: "Came out the same")
            },
            State(caption: "\(tone.label) · failed, can try again") {
                $0.preview(original: original, result: nil, selected: 1, engine: ready, error: RewriteError.timedOut)
            },
        ]

        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        for (page, start) in stride(from: 0, to: states.count, by: 4).enumerated() {
            let cells = states[start..<min(start + 4, states.count)].map { state -> AnyView in
                let model = PanelModel()
                state.setup(model)
                let expected = expectation(model)
                return AnyView(VStack(alignment: .leading, spacing: 2) {
                    Text(state.caption).font(.system(size: 15, weight: .bold)).foregroundStyle(.black)
                    Text(expected).font(.system(size: 13)).foregroundStyle(Color(white: 0.3))
                    PanelView(model: model)
                }.frame(width: 680, alignment: .topLeading))
            }
            let sheet = VStack(alignment: .leading, spacing: 18) {
                ForEach(Array(stride(from: 0, to: cells.count, by: 2)), id: \.self) { row in
                    HStack(alignment: .top, spacing: 20) {
                        cells[row]
                        if row + 1 < cells.count { cells[row + 1] }
                    }
                }
            }
            .padding(20)
            .background(DS.Surface.ground)
            ComponentSnapshot.renderPure(sheet, width: 1420, to: String(format: "%@/panel-states-%02d.png", dir, page + 1))
        }
    }

    /// What the rules say the row should show, to read against the drawing.
    private static func expectation(_ model: PanelModel) -> String {
        let footer = model.footer
        let app = switch footer.app {
        case .pastesInto(let name)?: "“\(name)” alone, no words before it"
        case .nothingToChange(let name)?: "“\(name)” alone, no words before it"
        case nil: "hidden"
        }
        return "expect: app \(app) · button \(footer.action) · note \(footer.note)"
    }
}
