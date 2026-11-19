import Foundation
import Testing
@testable import IrisCore

@Suite struct LaunchModeTests {
    @Test func noFlagIsTheApp() {
        #expect(LaunchMode(arguments: ["/Applications/Iris.app/Contents/MacOS/Iris"]) == .app)
        #expect(LaunchMode(arguments: ["Iris"]).answers == .model)
        #expect(LaunchMode(arguments: ["Iris"]).needsModel)
    }

    @Test func theChecksNeverUseTheModel() {
        #expect(LaunchMode(arguments: ["Iris", "--mock-answers"]) == .mockAnswers)
        #expect(LaunchMode(arguments: ["Iris", "--mock-answers"]).answers == .canned)
        #expect(LaunchMode(arguments: ["Iris", "--self-check"]) == .selfCheck)
        #expect(LaunchMode(arguments: ["Iris", "--self-check"]).answers == .never)
        #expect(!LaunchMode(arguments: ["Iris", "--self-check"]).needsModel)
    }

    @Test func aDrawingFlagTakesItsFolder() {
        #expect(LaunchMode(arguments: ["Iris", "--snapshot", "/tmp/iris"]) == .render(.snapshot, into: "/tmp/iris"))
        #expect(LaunchMode(arguments: ["Iris", "--panel-states", "/tmp/p"]) == .render(.panelStates, into: "/tmp/p"))
    }

    /// A drawing flag with no folder draws nothing and must not start the real app unnoticed.
    @Test func aDrawingFlagWithoutAFolderIsNamed() {
        #expect(LaunchMode.renderFlagMissingFolder(in: ["Iris", "--snapshot"]) == "--snapshot")
        #expect(LaunchMode.renderFlagMissingFolder(in: ["Iris", "--snapshot", "/tmp/iris"]) == nil)
        #expect(LaunchMode.renderFlagMissingFolder(in: ["Iris"]) == nil)
    }

    @Test func previewsTakeAnOptionalValue() {
        #expect(LaunchMode(arguments: ["Iris", "--preview-status-menu"]) == .statusMenuPreview(nil))
        #expect(LaunchMode(arguments: ["Iris", "--preview-status-menu", "check:new"]) == .statusMenuPreview("check:new"))
        #expect(LaunchMode(arguments: ["Iris", "--setup-preview"]) == .setupPreview(""))
        #expect(LaunchMode(arguments: ["Iris", "--toast-workbench", "fixed"]) == .toastWorkbench(start: "fixed"))
        #expect(LaunchMode(arguments: ["Iris", "--settings-preview"]) == .settingsPreview(nil))
        #expect(LaunchMode(arguments: ["Iris", "--settings-preview", "model"]) == .settingsPreview("model"))
        #expect(LaunchMode(arguments: ["Iris", "--preview-panel"]) == .panelPreview)
    }

    /// scripts/verify.sh puts each preview on the test display; the screen's name is never
    /// taken for the preview's own value.
    @Test func previewsOnAnotherDisplay() {
        let args = ["Iris", "--settings-preview", "--on-display", "Claude Test Display"]
        #expect(LaunchMode(arguments: args) == .settingsPreview(nil))
        #expect(LaunchMode.onDisplay(in: args) == "Claude Test Display")
        #expect(LaunchMode.onDisplay(in: ["Iris", "--settings-preview"]) == nil)
        #expect(LaunchMode(arguments: ["Iris", "--setup-preview", "1-allow", "--on-display", "X"]) == .setupPreview("1-allow"))
    }

    /// scripts/verify drives Iris with canned answers: a tone pressed there never wakes the model.
    @Test func theDrivenCopyNeverAsksTheModel() {
        #expect(LaunchMode(arguments: ["Iris", "--mock-answers", "--on-display", "X"]).answers == .canned)
    }

    /// The practice box runs in a process of its own, whatever else is on its command line.
    @Test func thePracticeBoxWinsOverEverythingElse() {
        #expect(LaunchMode(arguments: ["Iris", "--self-check", "--practice-box"]) == .practiceBox)
    }

    /// The scripts pass these same strings (scripts/lib.sh).
    @Test func theSameFlagsAsTheScripts() throws {
        let lib = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("scripts/lib.sh")
        let text = try String(contentsOf: lib, encoding: .utf8)
        func value(_ name: String) throws -> String {
            String(try #require(text.split(separator: "\n").first { $0.hasPrefix(name + "=") }).dropFirst(name.count + 1))
        }
        #expect(try value("IRIS_FLAG_MOCK_ANSWERS") == LaunchMode.mockAnswersFlag)
        #expect(try value("IRIS_FLAG_SELF_CHECK") == LaunchMode.selfCheckFlag)
        #expect(try value("IRIS_FLAG_PRACTICE_BOX") == LaunchMode.practiceBoxFlag)
    }
}
