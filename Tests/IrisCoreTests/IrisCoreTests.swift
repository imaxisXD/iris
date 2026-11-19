import Foundation
import Testing
@testable import IrisCore

@Suite struct ProtectorTests {
    @Test func aPlaceholderTheWriterTypedSurvives() throws {
        let protector = Protector()
        for draft in ["@bob see {{1}} please", "{{1}} and {{1}} then @a", "`x {{7}} y` and @a with {{2}}"] {
            let masked = protector.mask(draft)
            #expect(try protector.restore(masked.text, from: masked) == draft)
        }
    }

    let protector = Protector(glossary: ["Northwind", "OKR"])

    @Test func masksSlackTokensAndGlossary() throws {
        let input = "@ravi.k pls check #orders-team :eyes: https://example.com/x. `npm run build` for Northwind OKR"
        let masked = protector.mask(input)
        #expect(!masked.text.contains("@ravi"))
        #expect(!masked.text.contains("#orders-team"))
        #expect(!masked.text.contains(":eyes:"))
        #expect(!masked.text.contains("https://"))
        #expect(!masked.text.contains("npm run"))
        #expect(!masked.text.contains("Northwind"))
        #expect(!masked.text.contains("OKR"))
        // Trailing period stays outside the URL placeholder.
        #expect(masked.text.contains("}}."))
        #expect(try protector.restore(masked.text, from: masked) == input)
    }

    @Test func mentionDoesNotSwallowTrailingPunctuation() {
        let masked = protector.mask("thanks @ravi.")
        #expect(masked.originals == ["@ravi"])
        #expect(masked.text == "thanks {{1}}.")
    }

    @Test func rejectsDroppedOrDuplicatedPlaceholders() {
        let masked = protector.mask("ask @ravi and @meera")
        #expect(throws: Protector.RestoreError.self) { try protector.restore("ask {{1}} and someone", from: masked) }
        #expect(throws: Protector.RestoreError.self) { try protector.restore("{{1}} {{1}} {{2}}", from: masked) }
    }

    @Test func restoresTwoDigitPlaceholdersSafely() throws {
        let input = (1...12).map { ":e\($0):" }.joined(separator: " ")
        let masked = protector.mask(input)
        #expect(masked.originals.count == 12)
        #expect(try protector.restore(masked.text, from: masked) == input)
    }

    @Test func emailsAreNotMentions() {
        let masked = protector.mask("mail a@b.com")
        #expect(masked.originals == ["a@b.com"])
    }
}

@Suite struct WordDiffTests {
    @Test func groupsAdjacentChanges() {
        let segments = WordDiff.diff("and its look fine", "and it looks fine")
        #expect(segments == [.same("and "), .removed("its look"), .added("it looks"), .same(" fine")])
    }

    @Test func keepsApostropheWords() {
        #expect(WordDiff.tokenize("it's don’t") == ["it's", " ", "don’t"])
    }

    @Test func identicalTextIsAllSame() {
        #expect(WordDiff.diff("same text", "same text") == [.same("same text")])
    }
}

@Suite struct FixGuardTests {
    @Test func unchanged() {
        let v = FixGuard.evaluate(original: "Can you review this?", corrected: "Can you review this?")
        #expect(v.kind == .unchanged)
    }

    @Test func minorFix() {
        let v = FixGuard.evaluate(original: "i have checked the PR and its look fine",
                                  corrected: "I have checked the PR and it looks fine")
        #expect(v.kind == .minor)
        #expect(v.changedWords == 3)
    }

    @Test func heavyRewriteIsFlagged() {
        let v = FixGuard.evaluate(original: "can you check the order status today",
                                  corrected: "Would you kindly verify the current state of the purchase at your earliest convenience?")
        #expect(v.kind == .heavy)
    }

    @Test func droppingMostOfAMessageIsFlagged() {
        let v = FixGuard.evaluate(original: "please send the full report to the client before friday evening ok",
                                  corrected: "please send report ok")
        #expect(v.kind == .heavy)
        #expect(v.changedWords == 8)
    }

    @Test func aLeadInTheWriterTypedStays() {
        #expect(FixGuard.clean("Output: looks wrong on page 2", keeping: "Output: looks wrong on page 2") == "Output: looks wrong on page 2")
        #expect(FixGuard.clean("Output: it looks fine", keeping: "its look fine") == "it looks fine")
    }

    @Test func keepsOuterWhitespace() {
        let v = FixGuard.evaluate(original: "its fine\n", corrected: "it's fine")
        #expect(v.text == "it's fine\n")
        #expect(v.changedWords == 1)
    }
}

/// Answers each attempt in turn with the next of `outputs` (the last one again after that).
final class FakeGenerator: TextGenerator, @unchecked Sendable {
    let outputs: [String]
    private let lock = NSLock()
    private var calls = 0
    init(outputs: [String]) { self.outputs = outputs }

    func stream(_ turns: [PromptTurn], temperature: Float, maxTokens: Int, cacheKey: String?) -> AsyncThrowingStream<String, Error> {
        let index = lock.withLock { defer { calls += 1 }; return min(calls, outputs.count - 1) }
        let output = outputs[index]
        return AsyncThrowingStream { c in
            for word in output.split(separator: " ", omittingEmptySubsequences: false) { c.yield(String(word) + " ") }
            c.finish()
        }
    }
}

@Suite struct RewriterTests {
    @Test func fixRestoresMentionsAndDiffs() async throws {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["{{1}} it looks fine"]))
        var result: RewriteResult?
        for try await event in rewriter.run("@ravi its look fine", mode: .fix) {
            if case .finished(let r) = event { result = r }
        }
        #expect(result?.text == "@ravi it looks fine")
        #expect(result?.verdict == .minor)
    }

    @Test func retriesWhenPlaceholderDropped() async throws {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["it looks fine", "{{1}} it looks fine"]))
        var result: RewriteResult?
        for try await event in rewriter.run("@ravi its look fine", mode: .fix) {
            if case .finished(let r) = event { result = r }
        }
        #expect(result?.text == "@ravi it looks fine")
    }

    @Test func failsSafeWhenPlaceholdersLost() async {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["it looks fine"]))
        await #expect(throws: RewriteError.lostProtectedText) {
            for try await _ in rewriter.run("@ravi its look fine", mode: .fix) {}
        }
    }

    @Test func stripsWrappersTheModelAdds() async throws {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["Corrected: \"it's fine\""]))
        let result = try await rewriter.check("its fine\n", mode: .fix)
        #expect(result.text == "it's fine\n")
    }

    @Test func keepsQuotesTheWriterTyped() async throws {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["\"Ship it.\""]))
        #expect(try await rewriter.check("\"ship it.\"", mode: .fix).text == "\"Ship it.\"")
    }

    @Test func translatesADraftInAnotherLanguage() async throws {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["The order will arrive by tomorrow"]),
                                isForeign: { _ in true })
        let fix = try await rewriter.check("order kal tak aa jayega", mode: .fix)
        #expect(fix.translated)
        #expect(fix.verdict == nil)  // nothing in common with the draft to measure
        let tone = try await rewriter.check("order kal tak aa jayega", mode: .friendly)
        #expect(tone.translated)
    }

    @Test func countsChangedWordsFromTheDiff() async throws {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["I have checked the PR and it looks fine"]))
        #expect(try await rewriter.check("i have checked the PR and its look fine", mode: .fix).changedWords == 3)
    }

    @Test func languageCheckGetsMaskedText() async throws {
        actor Seen { var text = ""; func set(_ t: String) { text = t } }
        let seen = Seen()
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["{{1}} it looks fine"]),
                                isForeign: { await seen.set($0); return false })
        _ = try await rewriter.check("@ravi its look fine", mode: .fix)
        #expect(await seen.text == "{{1}} its look fine")
    }

    @Test func stripsThinkingBlocks() {
        #expect(Rewriter.stripThinking("<think>hmm</think>Hello") == "Hello")
        #expect(Rewriter.stripThinking("reasoning here</think>Hello") == "Hello")
    }

    @Test func rejectsVeryLongInput() async {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["ok"]))
        let long = String(repeating: "word ", count: 2000)
        await #expect(throws: RewriteError.tooLong(limit: Rewriter.maxInputCharacters)) {
            _ = try await rewriter.perform(long, mode: .fix) { _ in }
        }
    }

    @Test func timesOutAndLeavesTextAlone() async {
        struct Stuck: TextGenerator {
            func stream(_ turns: [PromptTurn], temperature: Float, maxTokens: Int, cacheKey: String?) -> AsyncThrowingStream<String, Error> {
                AsyncThrowingStream { c in
                    let t = Task { try? await Task.sleep(for: .seconds(60)); c.finish() }
                    c.onTermination = { _ in t.cancel() }
                }
            }
        }
        var rewriter = Rewriter(generator: Stuck())
        rewriter.timeLimitScale = 0.05
        let clock = ContinuousClock()
        let start = clock.now
        await #expect(throws: RewriteError.timedOut) {
            _ = try await rewriter.perform("hi there", mode: .fix) { _ in }
        }
        #expect(clock.now - start < .seconds(3))
    }

    @Test func cacheKeyIgnoresUserTextButNotInstructions() {
        let a = Rewriter.cacheKey(for: Rewriter.turns(for: "one", mode: .fix), mode: .fix)
        let b = Rewriter.cacheKey(for: Rewriter.turns(for: "two", mode: .fix), mode: .fix)
        #expect(a == b)
        let tone = Mode.builtInTones[0]
        let edited = Mode(id: tone.id, label: tone.label, instruction: tone.instruction + " More.")
        #expect(Rewriter.cacheKey(for: Rewriter.turns(for: "x", mode: tone), mode: tone)
                != Rewriter.cacheKey(for: Rewriter.turns(for: "x", mode: edited), mode: edited))
    }
}

@Suite struct OffTaskTests {
    @Test func dropsReasoningThatNeverClosed() {
        let out = "<|channel>thought\nThinking Process:\n1. **Analyze the input:** The input is"
        #expect(Rewriter.stripThinking(out).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }

    @Test func keepsAnswerAfterClosedReasoning() {
        #expect(Rewriter.stripThinking("<|channel>thought\nhmm<channel|>It looks fine.") == "It looks fine.")
    }

    @Test func gibberishThatGetsAnExplanationIsRefused() async {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: [
            "Thinking Process: 1. **Analyze the input:** The input is sdsaiofsi. 2. This appears to be gibberish or a typo, not a message.",
        ]))
        await #expect(throws: RewriteError.offTask) {
            _ = try await rewriter.perform("sdsaiofsi", mode: .fix) { _ in }
        }
    }

    @Test func noteToTheUserIsOffTask() {
        #expect(Rewriter.isOffTask("(No text provided to correct)", input: "sdsaiofsi", mode: .fix))
        #expect(!Rewriter.isOffTask("thanks", input: "thx", mode: .fix))
    }

    @Test func aFixThatOnlyLooksLikeAnExplanationIsKept() {
        #expect(!Rewriter.isOffTask("the user wants dark mode on the settings page", input: "user want dark mode on settings page", mode: .fix))
        #expect(!Rewriter.isOffTask("I think the input is wrong on line 3", input: "i think input is wrong on line 3", mode: .fix))
        #expect(!Rewriter.isOffTask("there is nothing to correct in this doc", input: "there is nothing to corect in this doc", mode: .fix))
        #expect(!Rewriter.isOffTask("The list doesn't appear to be sorted", input: "the list doesnt appear to be sorted", mode: .fix))
        #expect(!Rewriter.isOffTask("Please provide the invoice by Friday.", input: "send me invoice by friday", mode: Mode.builtInTones[0]))
    }

    @Test func anExplanationThatQuotesAShortDraftIsStillOffTask() {
        #expect(Rewriter.isOffTask("The input is gibberish.", input: "sdsaiofsi", mode: .fix))
        #expect(Rewriter.isOffTask("\"asdf qwer\" doesn't appear to be a real phrase", input: "asdf qwer", mode: .fix))
        #expect(Rewriter.isOffTask("Please provide the text you want me to rewrite.", input: "sdf gh jkl", mode: Mode.builtInTones[0]))
    }

    @Test func normalFixIsNotOffTask() {
        #expect(!Rewriter.isOffTask("hey, I have checked the PR and it looks fine", input: "hey i have checked the pr and its look fine", mode: .fix))
    }
}

@Suite struct EmojiTests {
    @Test func dropsEmojiTheModelAdded() {
        #expect(FixGuard.removingAddedEmoji(from: "Sure thing! Send it over when it's ready 😊", keeping: "send it when ready")
            == "Sure thing! Send it over when it's ready")
        #expect(FixGuard.removingAddedEmoji(from: "Shipped it 🚀 , thanks!", keeping: "shipped it") == "Shipped it, thanks!")
    }

    @Test func tidiesOnlyWhereAnEmojiWas() {
        #expect(FixGuard.removingAddedEmoji(from: "Thanks 🙂 see you :wave:", keeping: "thx see u :wave:") == "Thanks see you :wave:")
        #expect(FixGuard.removingAddedEmoji(from: "Nice 🎉\n    two  spaces :) ok", keeping: "x") == "Nice\n    two  spaces :) ok")
    }

    @Test func keepsTheWritersOwnEmojiAndPlainSymbols() {
        #expect(FixGuard.removingAddedEmoji(from: "Thanks 🙏 see you at 2", keeping: "thanks 🙏 see u at 2")
            == "Thanks 🙏 see you at 2")
        #expect(FixGuard.removingAddedEmoji(from: "Ticket #42 costs ©2026 3*4", keeping: "x") == "Ticket #42 costs ©2026 3*4")
        #expect(FixGuard.removingAddedEmoji(from: "done :tada:", keeping: "done :tada:") == "done :tada:")
    }

    @Test func toneRewritesComeBackWithoutAddedEmoji() async throws {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["Got it, I'll take a look this afternoon! 👀"]))
        let result = try await rewriter.perform("ok will check in afternoon", mode: Mode.builtInTones[1]) { _ in }
        #expect(result.text == "Got it, I'll take a look this afternoon!")
    }
}

@Suite struct DashTests {
    @Test func replacesDashesTheModelAdded() {
        #expect(FixGuard.replacingAddedDashes(in: "The build is stuck — can someone check?", keeping: "build stuck")
            == "The build is stuck, can someone check?")
        #expect(FixGuard.replacingAddedDashes(in: "Done—thanks!", keeping: "done thanks") == "Done, thanks!")
        #expect(FixGuard.replacingAddedDashes(in: "Heads-up – the meeting moved.", keeping: "meeting moved")
            == "Heads-up, the meeting moved.")
        #expect(FixGuard.replacingAddedDashes(in: "I'll check it —", keeping: "will check") == "I'll check it")
    }

    @Test func leavesEveryOtherCommaAndLineAlone() {
        #expect(FixGuard.replacingAddedDashes(in: "We sold 1,000 units — great job", keeping: "sold 1,000 units")
            == "We sold 1,000 units, great job")
        #expect(FixGuard.replacingAddedDashes(in: "See https://x.com/a,b and `f(a,b)` — ok, :thumbsup:", keeping: "x")
            == "See https://x.com/a,b and `f(a,b)`, ok, :thumbsup:")
        #expect(FixGuard.replacingAddedDashes(in: "Steps:\n— first\n— second", keeping: "x") == "Steps:\n— first\n— second")
        #expect(FixGuard.replacingAddedDashes(in: "Ready, — almost", keeping: "x") == "Ready, almost")
    }

    @Test func keepsTheWritersDashesRangesAndHyphens() {
        #expect(FixGuard.replacingAddedDashes(in: "Hours are 9–5, Mon–Fri", keeping: "hours 9-5") == "Hours are 9–5, Mon–Fri")
        #expect(FixGuard.replacingAddedDashes(in: "a well-known re-run", keeping: "x") == "a well-known re-run")
        #expect(FixGuard.replacingAddedDashes(in: "ok — will do", keeping: "ok — will do") == "ok — will do")
    }

    @Test func fixAndTonesBothComeBackWithoutAddedDashes() async throws {
        let fix = try await Rewriter(generator: FakeGenerator(outputs: ["the build is stuck — can u check"]))
            .perform("the build is stuck can u check", mode: .fix) { _ in }
        #expect(!fix.text.contains("—"))
        let tone = try await Rewriter(generator: FakeGenerator(outputs: ["Quick heads-up — the deploy is done!"]))
            .perform("deploy done", mode: Mode.builtInTones[1]) { _ in }
        #expect(tone.text == "Quick heads-up, the deploy is done!")
    }
}

@Suite struct EditorTextTests {
    @Test func removesSlackListMarkers() {
        // What Slack's message box reports for a three-item bullet list.
        #expect(EditorText.removingEditorMarkers("\u{E506}Going thriugh meganav stories\n\u{E506}NO\n\u{E506}NO")
            == "Going thriugh meganav stories\nNO\nNO")
        #expect(EditorText.removingEditorMarkers("see \u{FFFC} above") == "see  above")
    }

    @Test func keepsRealCharacters() {
        let text = "Ship it \u{F8FF} 🚀 — नमस्ते, “done” • item"
        #expect(EditorText.removingEditorMarkers(text) == text)
    }
}

@Suite struct FixNotesTests {
    @Test func namesTheSampleFixes() {
        let original = "hey, i have checked the PR and its look fine but one thing need to change before merge"
        let fixed = "hey, I have checked the PR and it looks fine, but one thing needs to change before merge"
        let reasons = FixNotes.changes(WordDiff.diff(original, fixed)).map(\.reason)
        #expect(reasons == [.capitalI, .verbForm, .comma, .agreement])
    }

    @Test func namesCommonKinds() {
        #expect(FixNotes.reason(from: "dont", to: "don't") == .apostrophe)
        #expect(FixNotes.reason(from: "", to: "the") == .article)
        #expect(FixNotes.reason(from: "about", to: "") == .preposition)
        #expect(FixNotes.reason(from: "recieve", to: "receive") == .spelling)
        #expect(FixNotes.reason(from: "went", to: "gone") == .wording)
        #expect(FixNotes.reason(from: "monday", to: "Monday") == .capitalLetter)
    }
}

@Suite struct StyleRulesTests {
    @Test func tonesCarryTheWritersRules() {
        let mode = Mode.builtInTones[0]
        let system = Prompts.toneSystem(mode, styleRules: ["keep my lowercase", "  ", "never \"kindly\""])
        #expect(system.contains("The writer's own style"))
        #expect(system.contains("- keep my lowercase"))
        #expect(system.contains("- never \"kindly\""))
        #expect(!Prompts.toneSystem(mode).contains("The writer's own style"))
    }

    @Test func rulesChangeTheCacheKey() {
        let mode = Mode.builtInTones[0]
        let plain = Rewriter.cacheKey(for: Rewriter.turns(for: "x", mode: mode), mode: mode)
        let styled = Rewriter.cacheKey(for: Rewriter.turns(for: "x", mode: mode, styleRules: ["no exclamation marks"]), mode: mode)
        #expect(plain != styled)
    }
}

@Suite struct WordWatchTests {
    let changes = FixNotes.changes(WordDiff.diff("Abhijyan, pls share the deck. one thing need fixing",
                                                 "Abhijeet, please share the deck. One thing needs fixing"))

    @Test func offersNamesNotGrammar() {
        let words = WordWatch.candidates(changes).map(\.from)
        #expect(words.contains("Abhijyan"))
        #expect(!words.contains("need"))
        #expect(WordWatch.candidates(changes, ignoring: ["Abhijyan"]).allSatisfy { $0.from != "Abhijyan" })
    }

    @Test func spotsAWordPutBack() {
        let candidates = WordWatch.candidates(changes)
        #expect(WordWatch.reverted(candidates, in: "Abhijeet, please share the deck.") == nil)
        #expect(WordWatch.reverted(candidates, in: "Abhijyan, please share the deck.")?.from == "Abhijyan")
        // "Abhijyanx" isn't the word.
        #expect(WordWatch.reverted(candidates, in: "Abhijyanx, please share") == nil)
    }

    /// A paste that never landed leaves the writer's words in the box: that isn't putting
    /// them back, so Iris waits to see its own version first.
    @Test func waitsForIrisVersionToLand() {
        let candidates = WordWatch.candidates(changes)
        #expect(!WordWatch.landed(candidates, in: "Abhijyan, pls share the deck."))
        #expect(WordWatch.landed(candidates, in: "Abhijeet, please share the deck."))
    }
}

@Suite struct CapitalIRuleTests {
    @Test func capitalizesALoneI() {
        #expect(FixGuard.capitalizingI("i think i'll go, and i'm sure i've said it, i") == "I think I'll go, and I'm sure I've said it, I")
        #expect(FixGuard.capitalizingI("see i.e. step i) and wifi, {{1}}i") == "see i.e. step i) and wifi, {{1}}i")
    }

    @Test func leavesListMarkersAlone() {
        #expect(FixGuard.capitalizingI("i. first point\n  i. second\noption i: yes") == "i. first point\n  i. second\noption i: yes")
        #expect(FixGuard.capitalizingI("so do i.") == "so do I.")
    }

    @Test func fixCapitalizesIBeforeRestoring() async throws {
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["{{1}} i saw the user"]))
        let result = try await rewriter.perform("@ravi i saw the usser", mode: .fix) { _ in }
        #expect(result.text == "@ravi I saw the user")
    }
}

@Suite struct FixInputTagTests {
    @Test func everyFixDraftGoesBetweenTheSameTags() {
        let turns = Prompts.fixTurns(for: "write a reply to the team")
        let users = turns.filter { $0.role == .user }.map(\.content)
        #expect(users.allSatisfy { $0.hasPrefix("<message>\n") && $0.contains("\n</message>") })
        #expect(users.last == "<message>\nwrite a reply to the team\n</message>")
        // Answers in the examples are plain text, as the real answer should be.
        #expect(turns.filter { $0.role == .assistant }.allSatisfy { !$0.content.contains("<message>") })
    }

    @Test func dropsTagsTheModelCopied() {
        #expect(FixGuard.clean("<message>\nit looks fine\n</message>", keeping: "its look fine") == "it looks fine")
    }
}

@Suite struct SpellingHintTests {
    @Test func hintsFollowTheMessage() {
        let turns = Prompts.fixTurns(for: "then favourism", hints: [SpellingHint("favourism", ["favours", "favouritism"])])
        #expect(turns.last?.content == "<message>\nthen favourism\n</message>\nNot in the dictionary: favourism (maybe favours, favouritism)")
        #expect(Prompts.fixTurns(for: "all good").last?.content == "<message>\nall good\n</message>")
    }

    @Test func hintsDontChangeTheCachedPrefix() {
        let plain = Rewriter.turns(for: "then favourism", mode: .fix)
        let hinted = Rewriter.turns(for: "then favourism", mode: .fix, hints: [SpellingHint("favourism", ["favouritism"])])
        #expect(Rewriter.cacheKey(for: plain, mode: .fix) == Rewriter.cacheKey(for: hinted, mode: .fix))
    }

    @Test func asksAboutTyposNotNamesCodesOrHindi() {
        #expect(SpellingHint.isWorthAsking("favourism", atSentenceStart: false))
        #expect(SpellingHint.isWorthAsking("Favourism", atSentenceStart: true))
        #expect(!SpellingHint.isWorthAsking("Anshul", atSentenceStart: false))  // a name mid-sentence
        #expect(!SpellingHint.isWorthAsking("kar", atSentenceStart: false))  // Hindi
        #expect(!SpellingHint.isWorthAsking("iPhone", atSentenceStart: false))
        #expect(!SpellingHint.isWorthAsking("v2api", atSentenceStart: false))
        #expect(!SpellingHint.isWorthAsking("ok", atSentenceStart: false))
    }

    @Test func findsSentenceStarts() {
        #expect(SpellingHint.startsSentence(after: ""))
        #expect(SpellingHint.startsSentence(after: "Done. "))
        #expect(SpellingHint.startsSentence(after: "really? \""))
        #expect(!SpellingHint.startsSentence(after: "ask "))
        #expect(SpellingHint.startsSentence(after: "hello team\n"))
        #expect(SpellingHint.startsSentence(after: "hello team\n\""))
    }

    @Test func fixPassesHintsForTwoWordsOrMore() async throws {
        actor Seen { var drafts: [String] = []; func add(_ d: String) { drafts.append(d) } }
        let seen = Seen()
        let rewriter = Rewriter(generator: FakeGenerator(outputs: ["then favouritism"]),
                                spellingHints: { draft in await seen.add(draft); return [SpellingHint("favourism", ["favouritism"])] })
        _ = try await rewriter.perform("then favourism", mode: .fix) { _ in }
        _ = try await rewriter.perform("favourism", mode: .fix) { _ in }
        #expect(await seen.drafts == ["then favourism"])
    }

    @Test func dropsTheHintLineIfTheModelCopiesIt() {
        #expect(FixGuard.clean("then favouritism\nNot in the dictionary: favourism (maybe favouritism)", keeping: "then favourism")
                == "then favouritism")
    }
}
