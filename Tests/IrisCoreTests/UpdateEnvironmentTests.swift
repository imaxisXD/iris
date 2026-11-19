import Foundation
import Testing
@testable import IrisCore

@Suite struct UpdateEnvironmentTests {
    /// The bug: Iris opened by ship.sh carried IRIS_COMMIT, so every update rebuilt that commit.
    @Test func leftoverBuildSettingsNeverReachTheUpdate() {
        let env = UpdateEnvironment.cleaned([
            "IRIS_COMMIT": "6eff07a", "IRIS_SOURCE_PATH": "/old", "SIGN_IDENTITY": "Someone", "HOME": "/Users/me",
        ])
        #expect(env["IRIS_COMMIT"] == nil)
        #expect(env["IRIS_SOURCE_PATH"] == nil)
        #expect(env["SIGN_IDENTITY"] == nil)
        #expect(env["HOME"] == "/Users/me")
        #expect(env["GIT_TERMINAL_PROMPT"] == "0")
    }

    /// The names match what scripts/lib.sh strips when it opens Iris.
    @Test func theSameNamesAsTheScripts() throws {
        let lib = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("scripts/lib.sh")
        let text = try String(contentsOf: lib, encoding: .utf8)
        let line = try #require(text.split(separator: "\n").first { $0.hasPrefix("IRIS_BUILD_VARS=(") })
        let names = line.dropFirst("IRIS_BUILD_VARS=(".count).dropLast().split(separator: " ").map(String.init)
        #expect(Set(names) == UpdateEnvironment.buildVariables)
    }
}
