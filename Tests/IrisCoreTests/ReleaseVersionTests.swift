import Testing
@testable import IrisCore

@Suite struct ReleaseVersionTests {
    @Test func newerReleasesAreNewer() {
        #expect(ReleaseVersion.isNewer("v0.2.0", than: "0.1.0"))
        #expect(ReleaseVersion.isNewer("0.10.0", than: "0.9.3"))   // numbers, not text
        #expect(ReleaseVersion.isNewer("1.0", than: "0.9.9"))
    }

    @Test func theSameOrOlderIsNot() {
        #expect(!ReleaseVersion.isNewer("v0.1.0", than: "0.1.0"))
        #expect(!ReleaseVersion.isNewer("0.1", than: "0.1.0"))
        #expect(!ReleaseVersion.isNewer("0.1.0", than: "0.2.0"))
    }

    @Test func unreadableTagsNeverNag() {
        #expect(!ReleaseVersion.isNewer("latest", than: "0.1.0"))
        #expect(!ReleaseVersion.isNewer("v0.2.0", than: "dev"))
        #expect(ReleaseVersion.parts("v1.2.0-beta") == [1, 2, 0])
    }
}
