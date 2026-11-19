import Testing
@testable import IrisCore

@Suite struct PanelLifeTests {
    /// Shown and opened, as after the shortcut and an answer.
    private func opened() -> PanelLife {
        var life = PanelLife()
        life.show()
        let opens = life.unfold()
        #expect(opens)
        return life
    }

    @Test func showsFoldedThenOpens() {
        var life = PanelLife()
        #expect(!life.isShowing)
        life.show()
        #expect(life.stage == .budding)
        #expect(life.isShowing)
        let opens = life.unfold()
        #expect(opens)
        #expect(life.stage == .unfolded)
        let reopens = life.unfold()
        #expect(!reopens)  // already open
    }

    @Test func closingAnOpenPanelFoldsItAway() {
        var life = opened()
        let close = life.close(immediately: false, reduceMotion: false)
        #expect(close == .fold(generation: 1))
        #expect(life.stage == .folding)
        let hides = life.foldFinished(1)
        #expect(hides)
        #expect(life.stage == .hidden)
    }

    /// The bug this was written for: the shortcut pressed while the panel folds away was taken
    /// as a second close (the panel was still on screen) and nothing opened.
    @Test func shortcutWhileFoldingOpensANewPanel() {
        var life = opened()
        _ = life.close(immediately: false, reduceMotion: false)
        #expect(!life.isShowing, "folding away doesn't count as showing, so the shortcut opens")
        life.show()
        #expect(life.isShowing)
        let oldFoldHides = life.foldFinished(1)
        #expect(!oldFoldHides, "the old fold must not hide the new panel")
        #expect(life.stage == .budding)
    }

    /// Found by the self-check: the folding panel still has the keyboard, so the shortcut must
    /// end the fold before reading the text box, or Iris reads its own panel.
    @Test func shortcutWhileFoldingEndsTheFoldFirst() {
        var life = opened()
        _ = life.close(immediately: false, reduceMotion: false)
        let ended = life.finishFold()
        #expect(ended)
        #expect(life.stage == .hidden)
        let hides = life.foldFinished(1)
        #expect(!hides, "the fold's own timer has nothing left to hide")
        let again = life.finishFold()
        #expect(!again)
    }

    @Test func goesAtOnceWhenPastingOrWithReduceMotion() {
        var pasting = opened()
        let close = pasting.close(immediately: true, reduceMotion: false)
        #expect(close == .hideNow)
        #expect(pasting.stage == .hidden)
        var still = opened()
        let stillClose = still.close(immediately: false, reduceMotion: true)
        #expect(stillClose == .hideNow)
    }

    @Test func aBudThatNeverOpenedGoesAtOnce() {
        var life = PanelLife()
        life.show()
        let close = life.close(immediately: false, reduceMotion: false)
        #expect(close == .hideNow)
        let lateUnfold = life.unfold()
        #expect(!lateUnfold, "a late unfold must not open a closed panel")
    }

    @Test func closingTwiceFinishesTheFold() {
        var life = opened()
        _ = life.close(immediately: false, reduceMotion: false)
        let second = life.close(immediately: false, reduceMotion: false)
        #expect(second == .hideNow)
        let third = life.close(immediately: false, reduceMotion: false)
        #expect(third == .nothing)
        let hides = life.foldFinished(1)
        #expect(!hides)
    }
}

@Suite struct ClosingPressTests {
    /// The bug this was written for: a long click on the trigger closed the menu on the press
    /// and, past a 0.6 s timer, opened it again on the release.
    @Test func releaseOfTheClosingPressDoesNotReopen() {
        var closing = ClosingPress()
        closing.closed(at: 10.02)  // the press began at 10.0 and was held for 1.5 s
        let same = closing.isClosingClick(pressBegan: 10.0)
        #expect(same)
    }

    /// Found by the self-check: the menu bar flower acts on the press, and macOS takes focus
    /// from the slip about 10 ms before that press's timestamp. Counting presses (the first
    /// fix) missed it, so a click on the flower closed the slip and opened it again.
    @Test func closedJustBeforeThePressStillCounts() {
        var closing = ClosingPress()
        closing.closed(at: 9.99)
        let same = closing.isClosingClick(pressBegan: 10.0)
        #expect(same)
    }

    @Test func closedWithEscThenClickedOnPurposeOpens() {
        var closing = ClosingPress()
        closing.closed(at: 10.0)
        let onPurpose = closing.isClosingClick(pressBegan: 10.6)
        #expect(!onPurpose)
    }

    @Test func theNextClickOpens() {
        var closing = ClosingPress()
        closing.closed(at: 10.02)
        let later = closing.isClosingClick(pressBegan: 11.0)
        #expect(!later)
    }

    @Test func theAnswerIsUsedUp() {
        var closing = ClosingPress()
        closing.closed(at: 10.02)
        _ = closing.isClosingClick(pressBegan: 10.0)
        let again = closing.isClosingClick(pressBegan: 10.0)
        #expect(!again, "a missed release must not swallow a later click")
    }

    @Test func aKeyOnTheTriggerIsNeverTheClosingClick() {
        var closing = ClosingPress()
        closing.closed(at: 10.0)
        let key = closing.isClosingClick(pressBegan: nil)
        #expect(!key)
    }
}
