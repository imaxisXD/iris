import Foundation
import Testing
@testable import IrisCore

@Suite struct QuickFixLandingTests {
    @Test func aFailedCheckThatCanBeRetriedOpensThePanel() {
        #expect(QuickFixLanding.failureOpensPanel(checkFailed: true, canRetry: true))
        // Too long, an empty box: trying again changes nothing, so a message says why.
        #expect(!QuickFixLanding.failureOpensPanel(checkFailed: true, canRetry: false))
        // The model couldn't be got (no internet): a message, though another try can work.
        #expect(!QuickFixLanding.failureOpensPanel(checkFailed: false, canRetry: true))
    }

    /// The bug this was written for: a Quick Fix that waited minutes for the model pasted
    /// (select all, paste) into whatever box had focus by then.
    @Test func aBoxThatChangedMeanwhileIsLeftAlone() {
        #expect(QuickFixLanding.decide(unchanged: false, heavy: false, canReplace: true, translated: false, stillThere: false) == .leftAlone)
        #expect(QuickFixLanding.decide(unchanged: false, heavy: false, canReplace: true, translated: false, stillThere: true) == .paste)
    }

    @Test func bigRewritesTranslationsAndUneditableTextGoToThePanel() {
        #expect(QuickFixLanding.decide(unchanged: false, heavy: true, canReplace: true, translated: false, stillThere: true) == .panel)
        #expect(QuickFixLanding.decide(unchanged: false, heavy: false, canReplace: false, translated: false, stillThere: true) == .panel)
        #expect(QuickFixLanding.decide(unchanged: false, heavy: false, canReplace: true, translated: true, stillThere: true) == .panel)
    }

    /// A box Iris can't read again: a quick answer may paste, a slow one mustn't, since you
    /// may have moved on to another box by then.
    @Test func onlyAQuickAnswerPastesBlind() {
        #expect(QuickFixLanding.unreadableStillThere(after: 1.5))
        #expect(!QuickFixLanding.unreadableStillThere(after: 30))
    }

    @Test func nothingToFixSaysSo() {
        #expect(QuickFixLanding.decide(unchanged: true, heavy: false, canReplace: true, translated: false, stillThere: false) == .clean)
    }
}

