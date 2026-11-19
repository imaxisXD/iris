import Testing
@testable import IrisCore

@Suite struct PanelFooterTests {
    static let stages: [PanelFooter.Stage] = [
        .working(fix: true), .working(fix: false),
        .done(fix: true, verdict: .minor), .done(fix: true, verdict: .heavy), .done(fix: true, verdict: .unchanged),
        .done(fix: true, verdict: nil),  // a translation
        .done(fix: false, verdict: nil),
        .failed(canRetry: true), .failed(canRetry: false),
    ]

    /// Every state the panel can be in: stage × app × editable × translated × notes × tones.
    static var everyState: [(PanelFooter.Stage, String?, Bool, Bool, Bool, String?, Int)] {
        var all: [(PanelFooter.Stage, String?, Bool, Bool, Bool, String?, Int)] = []
        for stage in stages {
            for app in ["Slack", "T3 Code (Nightly)", nil] as [String?] {
                for canReplace in [true, false] {
                    for translated in [false, true] {
                        for copied in [false, true] {
                            for retry in [nil, "came out the same"] as [String?] {
                                for tones in [1, 6] {
                                    all.append((stage, app, canReplace, translated, copied, retry, tones))
                                }
                            }
                        }
                    }
                }
            }
        }
        return all
    }

    @Test func theAppShowsWheneverThereIsAnAnswerToActOn() {
        for (stage, app, canReplace, translated, copied, retry, tones) in Self.everyState {
            let footer = PanelFooter(stage: stage, appName: app, canReplace: canReplace, translated: translated,
                                     copied: copied, retryNote: retry, toneCount: tones)
            let expected: PanelFooter.AppLabel? = switch stage {
            // Nothing to fix: Done, and the text in that app stays as it is.
            case .done(true, .unchanged): app.map(PanelFooter.AppLabel.nothingToChange)
            // An answer to paste, into a box that takes it.
            case .done: canReplace ? app.map(PanelFooter.AppLabel.pastesInto) : nil
            // No answer yet, or none coming.
            case .working, .failed: nil
            }
            #expect(footer.app == expected, "\(stage) app \(app ?? "none") canReplace \(canReplace)")
        }
    }

    @Test func fixAndATonePasteTheSameWay() {
        // The same answer state shows the app the same way in Fix and in any tone.
        for app in ["Slack", "T3 Code (Nightly)"] {
            for canReplace in [true, false] {
                let fix = PanelFooter(stage: .done(fix: true, verdict: .minor), appName: app, canReplace: canReplace,
                                      translated: false, copied: false, retryNote: nil, toneCount: 6)
                let tone = PanelFooter(stage: .done(fix: false, verdict: nil), appName: app, canReplace: canReplace,
                                       translated: false, copied: false, retryNote: nil, toneCount: 6)
                #expect(fix.app == tone.app)
                #expect(fix.action == tone.action)
            }
        }
        // A big rewrite or a translation still pastes, so it still says where.
        for (verdict, translated) in [(FixGuard.Verdict.Kind.heavy, false), (.minor, true)] {
            let footer = PanelFooter(stage: .done(fix: true, verdict: verdict), appName: "Slack", canReplace: true,
                                     translated: translated, copied: false, retryNote: nil, toneCount: 6)
            #expect(footer.app == .pastesInto("Slack"))
        }
    }

    @Test func nothingToFixSaysLooksGoodAndNamesTheAppWithoutPromisingAPaste() {
        let footer = PanelFooter(stage: .done(fix: true, verdict: .unchanged), appName: "Slack", canReplace: true,
                                 translated: false, copied: false, retryNote: nil, toneCount: 6)
        #expect(footer.note == .looksGood)
        #expect(footer.action == .done)
        #expect(footer.app == .nothingToChange("Slack"))
        // Text that can only be copied is still unchanged where it is.
        let copyOnly = PanelFooter(stage: .done(fix: true, verdict: .unchanged), appName: "Safari", canReplace: false,
                                   translated: false, copied: false, retryNote: nil, toneCount: 6)
        #expect(copyOnly.app == .nothingToChange("Safari"))
    }

    @Test func copyOnlyNeverNamesAnApp() {
        let footer = PanelFooter(stage: .done(fix: false, verdict: nil), appName: "Safari", canReplace: false,
                                 translated: false, copied: false, retryNote: nil, toneCount: 6)
        #expect(footer.action == .copy)
        #expect(footer.app == nil)
    }

    @Test func whileWorkingTheButtonSaysWhichAndNoKeysShow() {
        let fix = PanelFooter(stage: .working(fix: true), appName: "Slack", canReplace: true,
                              translated: false, copied: false, retryNote: nil, toneCount: 6)
        #expect(fix.action == .checking)
        #expect(fix.note == .none)
        let tone = PanelFooter(stage: .working(fix: false), appName: "Slack", canReplace: true,
                               translated: false, copied: false, retryNote: nil, toneCount: 6)
        #expect(tone.action == .writing)
    }

    @Test func notesComeInOrder() {
        func note(_ stage: PanelFooter.Stage, translated: Bool = false, copied: Bool = false, retry: String? = nil,
                  tones: Int = 6) -> PanelFooter.Note {
            PanelFooter(stage: stage, appName: "Slack", canReplace: true, translated: translated, copied: copied,
                        retryNote: retry, toneCount: tones).note
        }
        let done = PanelFooter.Stage.done(fix: true, verdict: .minor)
        #expect(note(done, translated: true, copied: true, retry: "x") == .copied)
        #expect(note(done, translated: true, retry: "x") == .retry("x"))
        #expect(note(done, translated: true) == .translated)
        #expect(note(.done(fix: true, verdict: .heavy)) == .bigRewrite)
        #expect(note(.done(fix: false, verdict: nil)) == .keys(nextTone: true, tryAgain: true))
        #expect(note(done, tones: 1) == .keys(nextTone: false, tryAgain: true))
        #expect(note(.failed(canRetry: true)) == .keys(nextTone: true, tryAgain: false))
    }

    @Test func aFailureOffersTryAgainOnlyWhenItCouldWork() {
        for canRetry in [true, false] {
            let footer = PanelFooter(stage: .failed(canRetry: canRetry), appName: "Slack", canReplace: true,
                                     translated: false, copied: false, retryNote: nil, toneCount: 6)
            #expect(footer.action == (canRetry ? .tryAgain : .none))
            #expect(footer.app == nil)
        }
    }
}
