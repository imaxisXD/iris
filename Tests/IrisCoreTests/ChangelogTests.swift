import Foundation
import Testing
@testable import IrisCore

@Suite struct ChangelogTests {
    let text = """
        Notes for whoever edits this file.

        # 0.2.0 · Second print
        date: 2026-12-01

        ## Faster fixes
        Fix comes back sooner
        on long messages.

        ## Your own tones
        Add as many as you like.

        - The menu bar flower is crisper.
        - Settings remembers its page.

        # v0.1.0 — First print
        date: 2026-11-07

        ## Fix and four tones
        One line.

        # not-a-version · Ignored
        ## Lost chapter
        """

    @Test func readsEachVersionNewestFirst() {
        let log = Changelog.parse(text)
        #expect(log.issues.map(\.version) == ["0.2.0", "0.1.0"])
        #expect(log.issues.map(\.title) == ["Second print", "First print"])
        #expect(log.issues.map(\.date) == ["2026-12-01", "2026-11-07"])
    }

    @Test func numbersIssuesFromTheOldest() {
        #expect(Changelog.parse(text).issues.map(\.number) == [2, 1])
    }

    @Test func chaptersKeepTheirLinesJoined() {
        let issue = Changelog.parse(text).issues[0]
        #expect(issue.chapters == [
            .init(title: "Faster fixes", detail: "Fix comes back sooner on long messages."),
            .init(title: "Your own tones", detail: "Add as many as you like."),
        ])
        #expect(issue.also == ["The menu bar flower is crisper.", "Settings remembers its page."])
        #expect(issue.changeCount == 4)
    }

    @Test func aBadHeadingDoesNotEatTheVersionBeforeIt() {
        let first = Changelog.parse(text).issues[1]
        #expect(first.chapters == [.init(title: "Fix and four tones", detail: "One line.")])
    }

    @Test func findsTheIssueForAVersion() {
        let log = Changelog.parse(text)
        #expect(log.issue(for: "v0.1.0")?.title == "First print")
        #expect(log.issue(for: "0.1")?.title == nil)
        #expect(log.issue(for: "dev") == nil)
    }

    @Test func emptyTextHasNoIssues() {
        #expect(Changelog.parse("").issues.isEmpty)
    }

    /// Two books with one version confuse the shelf (it tells books apart by version) and
    /// leave a ghost of the old front book standing over the new one.
    @Test func aVersionReadTwiceKeepsOnlyTheFirst() {
        let log = Changelog.parse("""
            # 0.2.0 · Turn the pages
            ## Books
            Open them.
            # v0.2.0 · Second print
            ## Faster
            Sooner.
            - A small thing.
            # 0.1.0 · First print
            """)
        #expect(log.issues.map(\.title) == ["Turn the pages", "First print"])
        #expect(log.issues[0].chapters.map(\.title) == ["Books"])
        #expect(log.issues[0].also.isEmpty)
        #expect(log.issues.map(\.number) == [2, 1])
    }
}

@Suite struct ProblemReportTests {
    @Test func opensANewIssueWithTheVersions() throws {
        let url = ProblemReport.url(repo: URL(string: "https://github.com/imaxisXD/iris")!,
                                    iris: "0.1.0 (129 · 5fc5784)", macOS: "26.1 (25B78)")
        #expect(url.absoluteString.hasPrefix("https://github.com/imaxisXD/iris/issues/new?body="))
        let body = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first?.value)
        #expect(body.contains("Iris 0.1.0 (129 · 5fc5784) · macOS 26.1 (25B78)"))
        #expect(body.contains("**What happened?**"))
    }

    @Test func aPlusSurvivesTheTrip() {
        let url = ProblemReport.url(repo: URL(string: "https://github.com/a/b")!, iris: "1.0+dev", macOS: "26")
        #expect(url.absoluteString.contains("1.0%2Bdev"))
    }
}

@Suite struct BundledChangelogTests {
    @Test func everyChapterInTheShippedFileHasItsLine() throws {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../Sources/Iris/Resources/Changelog.md")
        let log = Changelog.parse(try String(contentsOf: url, encoding: .utf8))
        #expect(!log.issues.isEmpty)
        for issue in log.issues {
            #expect(!issue.title.isEmpty)
            #expect(issue.date != nil)
            for chapter in issue.chapters { #expect(!chapter.detail.isEmpty, "\(issue.version) › \(chapter.title) has no line") }
        }
    }
}

@Suite struct ChangelogListTests {
    @Test func smallThingsAfterAChapterLeaveItsLineAlone() {
        let issue = Changelog.parse("# 1.0 · T\n## A\nLine a.\n\n- one\n- two\nstray words\n").issues[0]
        #expect(issue.chapters == [.init(title: "A", detail: "Line a.")])
        #expect(issue.also == ["one", "two"])
    }

    @Test func aPlainHyphenSeparatesTheTitleToo() {
        let issues = Changelog.parse("# 0.3.0 - Small repairs\n\n## One\nA line.\n").issues
        #expect(issues.first?.version == "0.3.0")
        #expect(issues.first?.title == "Small repairs")
    }
}

/// Settings › Updates prints one version's changes and offers the others as covers.
@Suite struct ChangelogReadingTests {
    let log = Changelog.parse("""
        # 0.3.0 · Three
        # 0.2.0 · Two
        # 0.1.0 · One
        """)

    @Test func nothingPickedShowsTheNewestAndOffersTheRestNewestFirst() throws {
        let reading = try #require(log.reading(nil))
        #expect(reading.shown.version == "0.3.0")
        #expect(reading.others.map(\.version) == ["0.2.0", "0.1.0"])
    }

    @Test func aPickedVersionIsShownAndTheNewestJoinsTheOthers() throws {
        let reading = try #require(log.reading("v0.2.0"))
        #expect(reading.shown.version == "0.2.0")
        #expect(reading.others.map(\.version) == ["0.3.0", "0.1.0"])
    }

    @Test func aVersionNoLongerInTheChangelogFallsBackToTheNewest() throws {
        #expect(try #require(log.reading("9.9.9")).shown.version == "0.3.0")
    }

    @Test func noVersionsReadsAsNothing() {
        #expect(Changelog(issues: []).reading(nil) == nil)
    }
}
