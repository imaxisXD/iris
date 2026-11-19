import Foundation

/// What changed in each version of Iris, read from the changelog bundled with the app
/// (`Sources/Iris/Resources/Changelog.md`). Each version is an issue of the Iris zine: a
/// title, a few chapters (the headline changes, each with one line) and the small things
/// "also in this version". Settings › Updates draws it.
///
/// The file, newest version first:
///
///     # 0.2.0 · A title for the issue
///     date: 2026-12-01
///
///     ## A headline change
///     One line saying what people get.
///
///     - A small thing, also in this version.
///
/// Anything else (a line before the first version, a comment) is ignored, so a typo never
/// hides a whole version.
public struct Changelog: Equatable, Sendable {
    public struct Issue: Equatable, Sendable, Identifiable {
        /// "0.1.0", without a leading v.
        public var version: String
        public var title: String
        /// "2026-11-07" as written in the file, or nil.
        public var date: String?
        public var chapters: [Chapter]
        public var also: [String]
        /// 1 for the first version ever, counting up: the issue's number on its cover.
        public var number: Int

        public var id: String { version }
        /// Every change in it: the chapters and the small things.
        public var changeCount: Int { chapters.count + also.count }

        public init(version: String, title: String, date: String? = nil, chapters: [Chapter] = [], also: [String] = [], number: Int = 1) {
            self.version = version
            self.title = title
            self.date = date
            self.chapters = chapters
            self.also = also
            self.number = number
        }
    }

    public struct Chapter: Equatable, Sendable {
        public var title: String
        public var detail: String
        public init(title: String, detail: String = "") {
            self.title = title
            self.detail = detail
        }
    }

    /// Newest first, as in the file.
    public var issues: [Issue]

    public init(issues: [Issue]) { self.issues = issues }

    /// The issue for `version` ("v0.1.0" and "0.1.0" alike), if the changelog has one.
    public func issue(for version: String) -> Issue? {
        issues.first { ReleaseVersion.same($0.version, version) }
    }

    /// What Settings › Updates shows: the version picked (the newest when none is, or when the
    /// one picked isn't in the changelog) with its changes, and every other version as a cover
    /// to pick, newest first. Nil with no versions at all.
    public func reading(_ picked: String?) -> (shown: Issue, others: [Issue])? {
        guard let newest = issues.first else { return nil }
        let shown = picked.flatMap(issue(for:)) ?? newest
        return (shown, issues.filter { $0.version != shown.version })
    }

    public static func parse(_ text: String) -> Changelog {
        var issues: [Issue] = []
        var detail: [String] = []
        /// A chapter heading was read and its lines are still coming.
        var inChapter = false
        /// Under a heading that isn't a version: its lines belong to no version.
        var skipping = false

        func closeChapter() {
            defer { detail = []; inChapter = false }
            guard inChapter, !issues.isEmpty, !issues[issues.count - 1].chapters.isEmpty else { return }
            let n = issues[issues.count - 1].chapters.count - 1
            issues[issues.count - 1].chapters[n].detail = detail.joined(separator: " ")
        }

        for raw in text.components(separatedBy: .newlines) {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.hasPrefix("## ") {
                closeChapter()
                guard !issues.isEmpty, !skipping else { continue }
                issues[issues.count - 1].chapters.append(Chapter(title: clean(line.dropFirst(3))))
                inChapter = true
            } else if line.hasPrefix("# ") {
                closeChapter()
                // "0.1.0 · First print": the version, then the title after a dot, dash, colon or
                // a hyphen with a space each side.
                let head = line.dropFirst(2).replacingOccurrences(of: " - ", with: " · ")[...]
                let cut = head.firstIndex { "·—–:".contains($0) } ?? head.endIndex
                var version = head[..<cut].trimmingCharacters(in: .whitespaces)
                if version.lowercased().hasPrefix("v") { version.removeFirst() }
                let title = cut < head.endIndex ? clean(head[head.index(after: cut)...]) : ""
                // A version that isn't one, or one already read (the first stays), is skipped
                // with everything under it: two books with one version can't share the shelf.
                skipping = ReleaseVersion.parts(version) == nil || issues.contains { ReleaseVersion.same($0.version, version) }
                guard !skipping else { continue }
                issues.append(Issue(version: version, title: title))
            } else if line.lowercased().hasPrefix("date:") {
                guard !issues.isEmpty, !skipping else { continue }
                let date = clean(line.dropFirst(5))
                issues[issues.count - 1].date = date.isEmpty ? nil : date
            } else if line.hasPrefix("- ") || line.hasPrefix("* ") {
                closeChapter()
                guard !issues.isEmpty, !skipping else { continue }
                issues[issues.count - 1].also.append(clean(line.dropFirst(2)))
            } else if line.isEmpty || line.hasPrefix("<!--") {
                continue
            } else if inChapter {
                detail.append(line)
            }
        }
        closeChapter()
        // Numbered from the oldest: the last in the file is No. 1.
        for i in issues.indices { issues[i].number = issues.count - i }
        return Changelog(issues: issues)
    }

    private static func clean<S: StringProtocol>(_ text: S) -> String {
        text.trimmingCharacters(in: .whitespaces)
    }
}

/// "Report a problem": a new GitHub issue already holding the facts a fix needs (which Iris,
/// which macOS), so the person only writes what happened.
public enum ProblemReport {
    public static func url(repo: URL, iris: String, macOS: String) -> URL {
        let body = """
            **What happened?**


            **What did you expect?**


            **How can I make it happen again?**
            1.

            ---
            Iris \(iris) · macOS \(macOS)
            """
        var parts = URLComponents(url: repo.appendingPathComponent("issues/new"), resolvingAgainstBaseURL: false)!
        parts.queryItems = [URLQueryItem(name: "body", value: body)]
        // URLComponents leaves "+" alone, which GitHub would read as a space.
        parts.percentEncodedQuery = parts.percentEncodedQuery?.replacingOccurrences(of: "+", with: "%2B")
        return parts.url!
    }
}
