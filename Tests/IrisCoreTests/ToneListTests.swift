import Testing
@testable import IrisCore

@Suite struct ToneListTests {
    private func custom(_ n: Int) -> [Mode] {
        (1...n).map { Mode(id: Mode.customPrefix + "\($0)", label: "Mine \($0)", instruction: "x") }
    }

    /// The bug: the panel drew six tabs, but Tab and the number keys reached every tone, so a
    /// seventh could be picked and never seen.
    @Test func thePanelOnlyHasTonesItCanShow() {
        let list = ToneList(custom: custom(4))
        #expect(list.shown.count == ToneList.panelRoom)
        #expect(list.shown.first == .fix)
        let last = list.all.filter { !list.hidden.contains($0.id) }.last!
        #expect(list.key(of: last.id) == nil)
        #expect(list.key(of: Mode.fix.id) == 1)
    }

    @Test func hiddenTonesStayOut() {
        let list = ToneList(hidden: [Mode.friendly.id])
        #expect(!list.shown.contains(.friendly))
        #expect(list.all.contains(.friendly))
    }

    @Test func startsOnTheAppsToneThenTheDefaultThenFix() {
        var list = ToneList(defaultID: Mode.concise.id, perApp: ["com.apple.mail": Mode.professional.id])
        #expect(list.starting(for: "com.apple.mail") == Mode.professional.id)
        #expect(list.starting(for: "com.tinyspeck.slackmacgap") == Mode.concise.id)
        list.hidden = [Mode.professional.id, Mode.concise.id]
        #expect(list.starting(for: "com.apple.mail") == Mode.fix.id)
    }

    @Test func removingAToneForgetsEverythingThatPointedAtIt() {
        var list = ToneList()
        list.add(name: "Customer", instruction: "polite", id: "abc")
        let id = Mode.customPrefix + "abc"
        list.defaultID = id
        list.perApp = ["com.apple.mail": id]
        list.hidden = [id]
        list.remove(id)
        #expect(list.custom.isEmpty && list.defaultID == Mode.fix.id && list.perApp.isEmpty && list.hidden.isEmpty)
        #expect(!list.order.contains(id))
    }

    @Test func movingSwapsWithTheNeighbour() {
        var list = ToneList()
        let first = list.all[0].id, second = list.all[1].id
        list.move(second, by: -1)
        #expect(list.all.prefix(2).map(\.id) == [second, first])
        list.move(list.all[0].id, by: -1)  // already first: nothing happens
        #expect(list.all.prefix(2).map(\.id) == [second, first])
    }
}
