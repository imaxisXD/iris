import Foundation
import Testing
@testable import IrisCore

/// The bug: Professional made a message polite by leaving out what it was for. "this is not
/// acceptable" disappeared, "I know you are busy and I hate to ask again" disappeared, and
/// "somebody should have told me" came back as "please let me know".
@Suite struct ToneMeaningTests {
    @Test func everyToneIsToldToLeaveNothingOut() {
        let custom = Mode(id: Mode.customPrefix + "1", label: "Customer update", instruction: "Polite, says what happens next.")
        for mode in Mode.builtInTones + [custom] {
            let system = Prompts.toneSystem(mode)
            #expect(system.contains("Leave nothing out."), "\(mode.label)")
            #expect(system.contains("opinion, feeling, and request"), "\(mode.label)")
        }
    }

    @Test func professionalIsToldWhatStays() {
        let instruction = Mode.professional.instruction
        for kept in ["a complaint", "a refusal", "an apology", "thanks", "a courtesy"] {
            #expect(instruction.contains(kept), "\(kept)")
        }
    }

    @Test func professionalShowsItsExamplesBeforeTheDraft() {
        let turns = Prompts.toneTurns(for: "pls check once", mode: .professional)
        #expect(turns.first?.role == .system)
        #expect(turns.last == PromptTurn(.user, "pls check once"))
        #expect(turns.count == 2 + Prompts.professionalExamples.count * 2)
        // A tone with no examples of its own is the rules and the draft.
        #expect(Prompts.toneTurns(for: "pls check once", mode: .concise).count == 2)
    }

    /// An example that loses something teaches the model to lose it: every number and every
    /// question in a draft is still in its rewrite.
    @Test func noExampleLeavesAnythingOut() {
        func numbers(_ s: String) -> [String] {
            s.split(whereSeparator: { !$0.isNumber }).map(String.init)
        }
        for (draft, rewrite) in Prompts.professionalExamples + Prompts.friendlyExamples + Prompts.clearerExamples {
            #expect(numbers(draft).sorted() == numbers(rewrite).sorted(), "\(draft)")
            if draft.contains("?") || draft.lowercased().hasPrefix("can ") || draft.contains(", can ") {
                #expect(rewrite.contains("?"), "\(draft)")
            }
            #expect(!rewrite.contains("—") && !rewrite.contains("–"), "\(draft)")
        }
    }

    /// The other way Professional went wrong: shown examples that copy the draft, the model
    /// copied the draft, and Professional read like Fix. Every example says its draft in other
    /// words: no more than two thirds of a rewrite's words come straight from its draft in order.
    @Test func professionalExamplesRewordTheirDrafts() {
        func words(_ s: String) -> [String] {
            s.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).map(String.init)
        }
        for (draft, rewrite) in Prompts.professionalExamples {
            let d = words(draft), r = words(rewrite)
            // Longest common subsequence: the words kept in the draft's order.
            var row = [Int](repeating: 0, count: r.count + 1)
            for word in d {
                var previous = 0
                for (j, other) in r.enumerated() {
                    let kept = row[j + 1]
                    row[j + 1] = word == other ? previous + 1 : max(row[j + 1], row[j])
                    previous = kept
                }
            }
            #expect(Double(row[r.count]) / Double(r.count) <= 2.0 / 3.0, "\(draft)")
        }
    }
}
