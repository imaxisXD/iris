import Foundation
import Testing
@testable import IrisCore

@Suite struct UpdateRunTests {
    private static func script(_ name: String) throws -> String {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        return try String(contentsOf: root.appendingPathComponent("scripts/" + name), encoding: .utf8)
    }

    @Test func aRunThatSaysNothingWrongHasNotFailed() {
        #expect(UpdateRun.failure(in: "") == nil)
        #expect(UpdateRun.failure(in: "── 2026-11-10 10:00:00 update\nAlready up to date.\nInstalled 1de210e to /Applications/Iris.app\n") == nil)
    }

    @Test func anotherUpdateRunningIsNotAFailureOfThisOne() {
        #expect(UpdateRun.failure(in: "Another update is running (remove .build/update.lock if it isn't).\nUpdate failed at line 55.\n") == .lockedOut)
    }

    /// build-app.sh lists the compiler's "error:" lines first, then says "Build failed".
    @Test func compilerErrorsReadAsABuildThatFailed() {
        let log = "main.swift:12:5: error: cannot find 'x' in scope\nBuild failed. Full log: .build/xcode-app.log\nUpdate failed at line 64.\n"
        #expect(UpdateRun.failure(in: log) == .buildFailed)
        #expect(UpdateRun.failure(in: log)?.message == "The new version didn't build.")
    }

    @Test func gitsOwnWordsStayInTheLog() {
        let failure = UpdateRun.failure(in: "error: Your local changes to the following files would be overwritten by merge:\nUpdate failed at line 44.\n")
        #expect(failure == .stopped)
        #expect(failure?.message == "The update stopped part way.")
        #expect(UpdateRun.failure(in: "fatal: unable to access 'https://github.com/': Could not resolve host\n") == .stopped)
    }

    @Test func aMissingRemoteSaysSoInTheScriptsWords() {
        #expect(UpdateRun.failure(in: "No GitHub remote set up for this checkout.\n")?.message == "No GitHub remote set up for this checkout.")
    }

    /// The lines read here are the ones the scripts write.
    @Test func theScriptsWriteTheLinesReadHere() throws {
        let update = try Self.script("update.sh"), build = try Self.script("build-app.sh")
        #expect(update.contains("echo \"\(UpdateRun.Marker.updateFailed) "))
        #expect(update.contains("echo \"\(UpdateRun.Marker.lockedOut) "))
        #expect(update.contains("echo \"\(UpdateRun.Marker.noRemote) "))
        #expect(build.contains("echo \"\(UpdateRun.Marker.buildFailed)."))
    }

    /// The lock, its age and the log are where scripts/lib.sh puts them.
    @Test func theSamePlacesAsTheScripts() throws {
        let lib = try Self.script("lib.sh")
        func value(_ name: String) throws -> String {
            let line = try #require(lib.split(separator: "\n").first { $0.hasPrefix(name + "=") })
            return line.dropFirst(name.count + 1).trimmingCharacters(in: CharacterSet(charactersIn: "\""))
        }
        #expect(try value("IRIS_UPDATE_LOCK") == UpdateRun.lockPath)
        #expect(try value("IRIS_UPDATE_LOCK_STALE_MINUTES") == String(UpdateRun.lockStaleMinutes))
        #expect(try value("IRIS_UPDATE_LOG") == "$HOME/" + UpdateRun.logPathInHome)
        #expect(UpdateRun.givesUpAfter == 15 * 60)
    }

    @Test func progressNeverClaimsDoneBeforeIrisReopens() {
        #expect(UpdateRun.progress(after: 0, expected: 75) == 0)
        let usual = UpdateRun.progress(after: 75, expected: 75)
        #expect(usual > 0.8 && usual < 0.9)
        #expect(UpdateRun.progress(after: 10_000, expected: 75) < 1)
        #expect(UpdateRun.progress(after: 30, expected: 75) < UpdateRun.progress(after: 60, expected: 75))
        // Never timed here: the usual time, not a division by zero.
        #expect(UpdateRun.progress(after: 75, expected: 0) == usual)
    }

    @Test func onlyAPlausibleTimeIsRemembered() {
        #expect(UpdateRun.isBelievable(seconds: 80))
        #expect(!UpdateRun.isBelievable(seconds: 3))        // a launch right after a failed start
        #expect(!UpdateRun.isBelievable(seconds: 86_400))   // an update that never finished, a day ago
    }

    @Test func theSameIssueWithOrWithoutTheV() {
        #expect(ReleaseVersion.same("v0.2.0", "0.2.0"))
        #expect(!ReleaseVersion.same("0.2", "0.2.0"))
        #expect(!ReleaseVersion.same("0.3.1", "0.3.2"))
        #expect(!ReleaseVersion.same("dev", "dev"))
    }

    @Test func onlyAGitHubPageIsOpened() {
        #expect(ReleaseVersion.releasePage("https://github.com/imaxisXD/iris/releases/tag/v0.2.0") != nil)
        #expect(ReleaseVersion.releasePage("http://github.com/imaxisXD/iris/releases/tag/v0.2.0") == nil)
        #expect(ReleaseVersion.releasePage("https://github.com.evil.example/x") == nil)
        #expect(ReleaseVersion.releasePage("file:///Applications/Calculator.app") == nil)
        #expect(ReleaseVersion.releasePage(nil) == nil)
    }
}
