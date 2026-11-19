import CoreGraphics
import Testing
@testable import IrisCore

@Suite struct PanelLayoutTests {
    let screen = CGRect(x: 0, y: 0, width: 1440, height: 900)
    let flower = PanelLayout.Flower(left: 44, right: 50, height: 118, headroom: 84)
    let paper = CGSize(width: 620, height: 220)

    private func place(_ box: CGRect, prefer: PanelLayout.Side = .above, keep: PanelLayout.Side? = nil,
                       paper: CGSize? = nil, in visible: CGRect? = nil) -> PanelLayout {
        PanelLayout.place(paper: paper ?? self.paper, by: box, prefer: prefer, keep: keep,
                          in: visible ?? screen, flower: flower)
    }

    /// A chat composer near the bottom of the screen, with room on its left.
    let composer = CGRect(x: 300, y: 60, width: 800, height: 80)

    @Test func sitsJustAboveTheBoxInLineWithIt() {
        let layout = place(composer)
        #expect(layout.side == .above)
        #expect(layout.paper.minY == composer.maxY + PanelLayout.gap)
        #expect(layout.paper.minX == composer.minX)
    }

    @Test func flowerStandsBesideThePaperNotUnderIt() {
        for prefer in [PanelLayout.Side.above, .below] {
            let box = prefer == .above ? composer : CGRect(x: 300, y: 700, width: 800, height: 80)
            let layout = place(box, prefer: prefer)
            #expect(layout.side == prefer)
            // The open bloom's right edge stays clear of the paper.
            #expect(layout.foot.x + flower.right + PanelLayout.flowerGap == layout.paper.minX)
            // And the bloom stands beside the paper, not floating above or below it.
            #expect(layout.foot.y + flower.height > layout.paper.minY)
            #expect(layout.foot.y < layout.paper.maxY)
        }
    }

    @Test func aboveTheFlowerStandsOnTheBoxsTopEdge() {
        let layout = place(composer)
        #expect(abs(layout.foot.y - composer.maxY) <= 2)
    }

    @Test func paperUnrollsFromTheBloomsHead() {
        let layout = place(composer)
        #expect(layout.hinge.x == layout.paper.minX)
        #expect(layout.hinge.y == layout.foot.y + flower.headroom)
    }

    @Test func belowHangsJustUnderTheBox() {
        let box = CGRect(x: 300, y: 600, width: 800, height: 80)
        let layout = place(box, prefer: .below)
        #expect(layout.side == .below)
        #expect(layout.paper.maxY == box.minY - PanelLayout.gap)
    }

    @Test func goesToTheOtherSideWhenThePreferredOneHasNoRoom() {
        // Prefers above, but the box is at the top of the screen.
        let high = CGRect(x: 300, y: 780, width: 800, height: 80)
        #expect(place(high, prefer: .above).side == .below)
        // Prefers below, but the box is at the bottom.
        #expect(place(composer, prefer: .below).side == .above)
    }

    @Test func neitherSideFitsTakesTheRoomierOneAndStaysOnScreen() {
        let short = CGRect(x: 0, y: 0, width: 1440, height: 400)
        let box = CGRect(x: 300, y: 150, width: 800, height: 80)  // 170 above, 150 below
        let layout = place(box, prefer: .below, paper: CGSize(width: 620, height: 300), in: short)
        #expect(layout.side == .above)
        #expect(layout.paper.maxY <= short.maxY - PanelLayout.edge)
        #expect(layout.paper.minY >= short.minY + PanelLayout.edge)
    }

    @Test func keepsItsSideWhileItGrows() {
        let first = place(composer)
        // Streaming text makes it so tall it no longer fits above, but it doesn't jump.
        let grown = place(composer, keep: first.side, paper: CGSize(width: 620, height: 800))
        #expect(grown.side == .above)
    }

    @Test func boxAtTheScreensLeftEdgeMovesThePaperRightSoTheFlowerShows() {
        let box = CGRect(x: 0, y: 60, width: 800, height: 80)
        let layout = place(box)
        #expect(layout.foot.x - flower.left >= screen.minX)
        #expect(layout.paper.minX > box.minX)
    }

    @Test func boxAtTheScreensRightEdgeKeepsThePaperOnScreenAndTheFlowerFollows() {
        let box = CGRect(x: 1100, y: 60, width: 330, height: 80)
        let layout = place(box)
        #expect(layout.paper.maxX <= screen.maxX - PanelLayout.edge)
        #expect(layout.foot.x + flower.right < layout.paper.minX)
    }

    @Test func worksOnASecondScreen() {
        let left = CGRect(x: -1920, y: 0, width: 1920, height: 1080)
        let box = CGRect(x: -1500, y: 60, width: 800, height: 80)
        let layout = place(box, in: left)
        #expect(layout.paper.minX == box.minX)
        #expect(left.contains(layout.paper))
    }
}

@Suite struct StandingPlaceTests {
    let screen = CGRect(x: 0, y: 0, width: 1440, height: 900)

    @Test func aTextBoxIsAPlaceToStand() {
        #expect(PanelLayout.canStand(on: CGRect(x: 200, y: 100, width: 600, height: 60), visible: screen))
    }

    /// A chat history or a document leaves no room above or below for the panel.
    @Test func aWholePageIsNot() {
        #expect(!PanelLayout.canStand(on: CGRect(x: 200, y: 100, width: 600, height: 500), visible: screen))
    }

    /// Chrome sometimes answers with a selection 30,000 points wide.
    @Test func nonsenseIsNot() {
        #expect(!PanelLayout.canStand(on: CGRect(x: 0, y: 100, width: 30_000, height: 20), visible: screen))
        #expect(!PanelLayout.canStand(on: CGRect(x: 5000, y: 5000, width: 100, height: 20), visible: screen))
    }
}
