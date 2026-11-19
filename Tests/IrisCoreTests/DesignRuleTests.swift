import Foundation
import Testing

/// Design rules that slipped back in once, checked in the source so they can't again.
@Suite struct DesignRuleTests {
    /// The stroked curl (`HandArrow`) is for marking up: pointing at a thing on the page. On a
    /// button or link it read as a hook ("Read what changed ⤴"), was fixed on Back, then came
    /// back on new buttons. Buttons and links take the printed `PenArrow` (DESIGN.md,
    /// "Patterns we don't use").
    @Test func theStrokedCurlOnlyMarksThingsUp() throws {
        let sources = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../Sources/Iris")
        // Where it may be drawn: the hand-lettered label pointing at a box, the panel's red pen.
        let markingUp: Set = ["Zine.swift", "PanelView.swift"]
        let files = try FileManager.default.contentsOfDirectory(at: sources, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "swift" }
        #expect(!files.isEmpty)
        for file in files where !markingUp.contains(file.lastPathComponent) {
            let lines = try String(contentsOf: file, encoding: .utf8).components(separatedBy: .newlines)
            for (i, line) in lines.enumerated() where line.contains("HandArrow(") {
                Issue.record("\(file.lastPathComponent):\(i + 1) draws the stroked curl. On a button or link use PenArrow (right: true for forward).")
            }
        }
    }

    /// Every primary button is the same stroke of one brush. Seeded by its label, each one
    /// grew its own ends and streaks, and the screens read as having different buttons
    /// (DESIGN.md, Components, "The next step").
    @Test func everyPrimaryButtonIsTheSameBrush() throws {
        let file = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("../../Sources/Iris/Components.swift")
        let lines = try String(contentsOf: file, encoding: .utf8).components(separatedBy: .newlines)
        let slabs = lines.enumerated().filter { $0.element.contains("BrushSlab(") }
        #expect(!slabs.isEmpty)
        for (i, line) in slabs where !line.contains("seed: Self.brush") {
            Issue.record("Components.swift:\(i + 1) seeds a primary button's brush by something other than Self.brush.")
        }
    }
}
