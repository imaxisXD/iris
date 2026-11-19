import Testing
@testable import IrisCore

@Suite struct ListKeysTests {
    @Test func movesOneRowAndStopsAtTheEnds() {
        #expect(ListKeys.step(from: 1, count: 4, down: true) == 2)
        #expect(ListKeys.step(from: 3, count: 4, down: true) == 3)
        #expect(ListKeys.step(from: 1, count: 4, down: false) == 0)
        #expect(ListKeys.step(from: 0, count: 4, down: false) == 0)
    }

    @Test func startsAtTheTopGoingDownAndTheBottomGoingUp() {
        #expect(ListKeys.step(from: nil, count: 4, down: true) == 0)
        #expect(ListKeys.step(from: nil, count: 4, down: false) == 3)
    }

    /// The slip and the dropdown both indexed `ids[count - 1]`, which traps on an empty list.
    @Test func anEmptyListHasNoRowToGoTo() {
        #expect(ListKeys.step(from: nil, count: 0, down: true) == nil)
        #expect(ListKeys.step(from: nil, count: 0, down: false) == nil)
    }
}
