import Foundation
import Testing
@testable import IrisCore
@testable import IrisEngine

/// Which models people see and start on, and how each is asked. The trap these guard: an
/// update that leaves someone who set Iris up before on a model that isn't on their Mac.
@Suite struct ModelChoiceTests {
    @Test func newMacsAreOfferedOnlyIrisOwnModels() {
        #expect(ModelChoice.offered(onDisk: { _ in false }) == [.best, .lighter])
    }

    @Test func anEarlierModelOnDiskStaysListed() {
        #expect(ModelChoice.offered(onDisk: { $0 == .e4b }) == [.best, .lighter, .e4b])
        #expect(ModelChoice.offered(onDisk: { $0 == .e2b || $0 == .best }) == [.best, .lighter, .e2b])
    }

    @Test func recommendationFollowsMemory() {
        #expect(ModelChoice.recommended(memoryGB: 8) == .lighter)
        #expect(ModelChoice.recommended(memoryGB: 16) == .best)
        #expect(ModelChoice.recommended(memoryGB: 36) == .best)
    }

    @Test func someoneNewStartsOnTheOneThatSuitsTheirMac() {
        #expect(ModelChoice.starting(setUpBefore: false, onDisk: { _ in false }, memoryGB: 8) == .lighter)
        #expect(ModelChoice.starting(setUpBefore: false, onDisk: { _ in false }, memoryGB: 32) == .best)
        // Not set up yet, even with an old download lying around: the new list, the new pick.
        #expect(ModelChoice.starting(setUpBefore: false, onDisk: { $0 == .e4b }, memoryGB: 32) == .best)
    }

    /// The update must not take away the model they have: they keep Google's until they choose.
    @Test func someoneSetUpBeforeKeepsTheModelOnTheirMac() {
        #expect(ModelChoice.starting(setUpBefore: true, onDisk: { $0 == .e4b }, memoryGB: 32) == .e4b)
        #expect(ModelChoice.starting(setUpBefore: true, onDisk: { $0 == .e2b }, memoryGB: 16) == .e2b)
        // They'd switched to the other size at some point and that's the one on disk.
        #expect(ModelChoice.starting(setUpBefore: true, onDisk: { $0 == .e2b }, memoryGB: 32) == .e2b)
        // Nothing on disk any more: no point holding on to a model they'd have to fetch again.
        #expect(ModelChoice.starting(setUpBefore: true, onDisk: { _ in false }, memoryGB: 8) == .lighter)
    }

    @Test func trainedModelsAreAskedTheWayTheyWereTaught() {
        #expect(ModelChoice.best.promptStyle == .trained)
        #expect(ModelChoice.lighter.promptStyle == .trained)
        #expect(ModelChoice.e4b.promptStyle == .taught)
        #expect(ModelChoice.e2b.promptStyle == .taught)
    }

    /// Iris's own models and Google's earlier ones share drafters. Deleting one must leave the
    /// drafter for the other, or the model left behind loses its speed-up.
    @Test func deletingKeepsADrafterAnotherModelUses() {
        let best = ModelChoice.best.cacheDirectories(keepingFor: { $0 == .e4b })
        #expect(best.count == 1)
        #expect(best[0].lastPathComponent == "models--ImAXISOG--iris-gemma-4-e4b-it-mlx")
        #expect(ModelChoice.e4b.cacheDirectories(keepingFor: { $0 == .best }).count == 1)
        // Nothing else uses it: the drafter goes too.
        #expect(ModelChoice.best.cacheDirectories(keepingFor: { _ in false }).count == 2)
        // Lighter's drafter is E2B's, not E4B's.
        #expect(ModelChoice.lighter.cacheDirectories(keepingFor: { $0 == .e4b }).count == 2)
    }

    @Test func irisModelsArePinnedToTheTestedUpload() {
        for choice in [ModelChoice.best, .lighter] {
            if case .id(let id, let revision) = choice.configuration.id {
                #expect(id.hasPrefix("ImAXISOG/iris-gemma-4-"))
                #expect(revision.count == 40)
            } else {
                Issue.record("\(choice) isn't a Hugging Face model")
            }
        }
    }
}

@Suite struct TrainedPromptTests {
    @Test func trainedFixIsShortWithNoExamplesOrHints() {
        let turns = Prompts.fixTurns(for: "then favourism", hints: [SpellingHint("favourism", ["favouritism"])], style: .trained)
        #expect(turns.count == 2)
        #expect(turns[0].content.hasPrefix("Proofread the message between <message> and </message>."))
        #expect(turns[1].content == "<message>\nthen favourism\n</message>")
    }

    @Test func trainedToneIsShortWithNoExamples() {
        let turns = Prompts.toneTurns(for: "hey", mode: .friendly, style: .trained)
        #expect(turns.count == 2)
        #expect(turns[0].content.hasPrefix("Rewrite the message in a friendly tone."))
        // The writer's own style rules still reach a trained model.
        #expect(Prompts.toneSystem(.friendly, styleRules: ["keep my lowercase"], style: .trained).contains("keep my lowercase"))
    }

    @Test func taughtPromptsAreUnchangedByDefault() {
        #expect(Prompts.toneTurns(for: "hey", mode: .friendly).count > 2)
        #expect(Prompts.fixTurns(for: "hey").count > 2)
    }

    /// Trained models were judged with tones at temperature 0; Google's keep 0.7.
    @Test func trainedTonesStartAtZeroTemperature() async throws {
        final class Recorder: TextGenerator, @unchecked Sendable {
            var temperatures: [Float] = []
            func stream(_ turns: [PromptTurn], temperature: Float, maxTokens: Int, cacheKey: String?) -> AsyncThrowingStream<String, Error> {
                temperatures.append(temperature)
                return AsyncThrowingStream { $0.yield("Hey there, how are you?"); $0.finish() }
            }
        }
        for (style, expected) in [(PromptStyle.trained, Float(0)), (.taught, Float(0.7))] {
            let recorder = Recorder()
            _ = try await Rewriter(generator: recorder, promptStyle: style).perform("hey how are you", mode: .friendly) { _ in }
            #expect(recorder.temperatures.first == expected)
        }
    }
}
