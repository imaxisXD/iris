import Testing
@testable import IrisCore

@Suite struct SetupWordsTests {
    private func progress(trusted: Bool = true, onDisk: Bool = false, _ activity: SetupProgress.ModelActivity = .none,
                          tried: Bool = false, before: Bool = false) -> SetupProgress {
        SetupProgress(.init(trusted: trusted, modelOnDisk: onDisk, activity: activity, tried: tried, finishedBefore: before))
    }

    /// The slip, setup's list and VoiceOver said each step their own way ("allow Iris." in one,
    /// "allow Iris to read the box you type in." in another). One line per step now.
    @Test func theSlipAndVoiceOverSayTheSameStep() {
        let allow = progress(trusted: false)
        #expect(allow.slipLine == allow.announcement)
        #expect(allow.slipLine == "Step 1 of 3: allow Iris to read the box you type in.")
        let model = progress()
        #expect(model.slipLine == model.announcement)
        #expect(progress(onDisk: true).slipLine == "Step 3 of 3: try it once.")
        #expect(progress(onDisk: true, tried: true).slipLine == nil)
    }

    @Test func theSlipSaysWhatWentWrongWithTheModel() {
        #expect(progress(.failed(.offline)).slipLine == ModelFailure.offline.sentence)
        #expect(progress(.downloading(0.4)).slipLine == "Step 2 of 3: getting the model.")
        #expect(progress(before: true).slipLine == "The model isn't on this Mac any more. Download it again.")
    }

    /// Allowed with no model yet: setup showed an open bloom while the slip showed a closed bud.
    /// Both draw `garden` now.
    @Test func theGardenFollowsSetup() {
        #expect(progress(trusted: false, .downloading(0.5)).garden == .bud)
        #expect(progress().garden == .allowed)
        #expect(progress(.downloading(0.5)).garden == .watering)
        #expect(progress(onDisk: true, .loading).garden == .warming)
        #expect(progress(onDisk: true).garden == .ready)
        #expect(progress(onDisk: true, tried: true).garden == .tried)
    }
}
