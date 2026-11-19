import Testing
@testable import IrisCore

@Suite struct ToneLabelTests {
    /// The bug: a Professional rewrite showed as "Professional: Hi @lead, ..." in the panel.
    @Test func theTonesNameBeforeTheAnswerIsDropped() {
        let draft = "hey, can u check the PR"
        for answer in ["Professional: Hi, could you review the PR?", "professional tone: Hi, could you review the PR?",
                       "**Professional:** Hi, could you review the PR?", "Professional version:\nHi, could you review the PR?"] {
            #expect(FixGuard.removingLabel(answer, label: "Professional", keeping: draft) == "Hi, could you review the PR?")
        }
    }

    @Test func aDraftThatStartsWithTheWordKeepsIt() {
        let draft = "Professional: please review"
        #expect(FixGuard.removingLabel("Professional: Please review.", label: "Professional", keeping: draft) == "Professional: Please review.")
    }

    @Test func theWordInsideTheAnswerIsLeftAlone() {
        let answer = "Thanks for keeping it professional: I'll review it today."
        #expect(FixGuard.removingLabel(answer, label: "Professional", keeping: "thx") == answer)
        #expect(FixGuard.removingLabel("Shorter is better: ship it.", label: "Shorter", keeping: "x") == "Shorter is better: ship it.")
    }
}
