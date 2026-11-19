import Testing
@testable import IrisCore

@Suite struct PanelMotionTests {
    @Test func theBudHasItsMomentThenTheBloomThenThePanel() {
        // An answer in at once: the bud's whole moment, then a beat after the bloom.
        #expect(PanelMotion.unrollDelay(waited: 0, bloomed: true, full: true) == 0.6 + PanelMotion.afterBloom(full: true))
        // An answer that took longer than the moment: just the beat after the bloom.
        #expect(PanelMotion.unrollDelay(waited: 2, bloomed: true, full: true) == PanelMotion.afterBloom(full: true))
        // Still waiting (the model is waking): open once the moment's over, no bloom to wait for.
        #expect(PanelMotion.unrollDelay(waited: 0.2, bloomed: false, full: false) == 0)
    }
}
