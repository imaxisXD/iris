import Foundation
import Testing
import MLX
@testable import MLXLMCommon
@testable import MLXVLM
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
        for choice in ModelChoice.allCases {
            if case .id(_, let revision) = choice.configuration.id {
                #expect(revision.count == 40, "\(choice) model revision isn't pinned")
            } else {
                Issue.record("\(choice) isn't a Hugging Face model")
            }
            if case .id(_, let revision) = choice.drafter.id {
                #expect(revision.count == 40, "\(choice) drafter revision isn't pinned")
            } else {
                Issue.record("\(choice) drafter isn't a Hugging Face model")
            }
        }
        for choice in [ModelChoice.best, .lighter] {
            if case .id(let id, _) = choice.configuration.id {
                #expect(id.hasPrefix("ImAXISOG/iris-gemma-4-"))
            }
        }
    }

    @Test func prefixStoreClearPreventsStaleResurrection() {
        let store = PrefixStore(capacity: 4)
        let genBeforeClear = store.currentGeneration()
        store.clear()
        let entry = PrefixStore.Entry(tokens: [1, 2, 3], suffixCount: 0, cache: [])
        store.store(entry, forKey: "fix:abc", ifGeneration: genBeforeClear)
        #expect(store["fix:abc"] == nil)
        store.store(entry, forKey: "fix:abc", ifGeneration: store.currentGeneration())
        #expect(store["fix:abc"]?.tokens == [1, 2, 3])
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

@Suite struct HardeningTests {
    @Test func suppressTokensClampsOutOfRangeIDsWithoutTrapping() {
        #expect(SuppressTokensProcessor(tokenIds: [-1, Int(Int32.max) + 1, Int(UInt32.max) + 1]) == nil)
        let mixed = SuppressTokensProcessor(tokenIds: [42, Int(Int32.max) + 5, Int(UInt32.max) + 10])
        #expect(mixed?.suppressedIds == [42])
    }

    @Test func wordTableFileIsScopedPerTask() async {
        let urlA = URL(fileURLWithPath: "/tmp/model-a.safetensors")
        let urlB = URL(fileURLWithPath: "/tmp/model-b.safetensors")
        await withTaskGroup(of: Void.self) { group in
            group.addTask {
                await Gemma4TextOnly.withWordTableFile(urlA) {
                    try? await Task.sleep(for: .milliseconds(20))
                    #expect(Gemma4TextOnly.wordTableFile == urlA)
                }
            }
            group.addTask {
                await Gemma4TextOnly.withWordTableFile(urlB) {
                    try? await Task.sleep(for: .milliseconds(5))
                    #expect(Gemma4TextOnly.wordTableFile == urlB)
                }
            }
        }
        #expect(Gemma4TextOnly.wordTableFile == nil)
    }

    /// Writes a minimal valid 4-bit quantized safetensors table with `rows` rows of `rowWidth = 32`
    /// where row `r` dequantizes to all `Float(r)`.
    private static func writeSyntheticWordTable(rows: Int, truncateBy: Int = 0) throws -> URL {
        // 4-bit, groupSize = 32 -> weight: 4 U32 cols (16 B/row), scales: 1 F32 col (4 B/row), biases: 1 F32 col (4 B/row).
        // With weights = 0, scales = 1.0, biases = Float(r), every element in row r dequantizes to Float(r).
        let wBytes = rows * 16
        let sBytes = rows * 4
        let bBytes = rows * 4
        let headerObj: [String: Any] = [
            "language_model.model.embed_tokens_per_layer.weight": [
                "dtype": "U32", "shape": [rows, 4], "data_offsets": [0, wBytes],
            ],
            "language_model.model.embed_tokens_per_layer.scales": [
                "dtype": "F32", "shape": [rows, 1], "data_offsets": [wBytes, wBytes + sBytes],
            ],
            "language_model.model.embed_tokens_per_layer.biases": [
                "dtype": "F32", "shape": [rows, 1], "data_offsets": [wBytes + sBytes, wBytes + sBytes + bBytes],
            ],
        ]
        let headerData = try JSONSerialization.data(withJSONObject: headerObj, options: [.sortedKeys])
        var fileData = Data()
        var headerLen = UInt64(headerData.count).littleEndian
        fileData.append(Data(bytes: &headerLen, count: 8))
        fileData.append(headerData)
        fileData.append(Data(count: wBytes))
        for _ in 0..<rows {
            var one = Float(1.0).bitPattern.littleEndian
            fileData.append(Data(bytes: &one, count: 4))
        }
        for r in 0..<rows {
            var bias = Float(r).bitPattern.littleEndian
            fileData.append(Data(bytes: &bias, count: 4))
        }
        if truncateBy > 0, fileData.count > truncateBy {
            fileData.removeLast(truncateBy)
        }
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("iris-wordtable-\(UUID().uuidString).safetensors")
        try fileData.write(to: url)
        return url
    }

    @Test func wordTableRejectsMalformedAndTruncatedFiles() throws {
        // headerLength == 0
        let zeroHeader = FileManager.default.temporaryDirectory.appendingPathComponent("iris-zero-\(UUID().uuidString).safetensors")
        defer { try? FileManager.default.removeItem(at: zeroHeader) }
        try Data(count: 16).write(to: zeroHeader)
        #expect(Gemma4WordTableOnDisk(file: zeroHeader, rowWidth: 32, rowCount: 4) == nil)

        // Truncated data region
        let truncated = try Self.writeSyntheticWordTable(rows: 8, truncateBy: 4)
        defer { try? FileManager.default.removeItem(at: truncated) }
        #expect(Gemma4WordTableOnDisk(file: truncated, rowWidth: 32, rowCount: 8) == nil)

        // Fewer rows than model vocabulary requires
        let tooFew = try Self.writeSyntheticWordTable(rows: 8)
        defer { try? FileManager.default.removeItem(at: tooFew) }
        #expect(Gemma4WordTableOnDisk(file: tooFew, rowWidth: 32, rowCount: 16) == nil)
    }

    @Test func wordTableRowCacheDoesNotCorruptHitEvictedInSameLookup() throws {
        // Fill all 2048 slots (0..<2048), so nextSlot wraps to 0 (holding token 0).
        // Then look up [0, 2048] together: 2048 evicts slot 0 during the same call; row 0
        // must still return token 0's values (0.0), not token 2048's (2048.0).
        let url = try Self.writeSyntheticWordTable(rows: 2050)
        defer { try? FileManager.default.removeItem(at: url) }
        let table = try #require(Gemma4WordTableOnDisk(file: url, rowWidth: 32, rowCount: 2050))
        let warmup = MLXArray((0..<2048).map(Int32.init))
        _ = table.lookup(warmup)

        let pair = table.lookup(MLXArray([Int32(0), Int32(2048)])).asType(.float32)
        eval(pair)
        let row0 = pair[0, 0].item(Float.self)
        let row1 = pair[1, 0].item(Float.self)
        #expect(row0 == 0.0)
        #expect(row1 == 2048.0)
    }
}
