import Testing
@testable import IrisCore

@Suite struct BudArrivalTests {
    @Test func theFirstFiveArrivalsArePlayedInFull() {
        for n in 0..<BudArrival.firstFull { #expect(BudArrival.isFull(arrivals: n, firstToday: false, roll: 1)) }
        #expect(!BudArrival.isFull(arrivals: BudArrival.firstFull, firstToday: false, roll: 1))
    }

    @Test func theFirstOfTheDayAndNowAndThen() {
        #expect(BudArrival.isFull(arrivals: 40, firstToday: true, roll: 1))
        #expect(BudArrival.isFull(arrivals: 40, firstToday: false, roll: 0))
        #expect(!BudArrival.isFull(arrivals: 40, firstToday: false, roll: 7))
    }
}
