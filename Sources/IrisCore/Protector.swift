import Foundation

/// Swaps things the model must never touch (Slack mentions, channels, emoji
/// shortcodes, links, code, emails, glossary terms) for numbered placeholders
/// before generation, and puts them back afterwards.
///
/// The model only ever sees `{{1}}`, `{{2}}`, ... so it cannot "fix" a name,
/// break a link, or reword a code span.
public struct Protector: Sendable {
    public struct Masked: Sendable, Equatable {
        public let text: String
        public let originals: [String]  // index 0 is placeholder {{1}}
    }

    public enum RestoreError: Error, Equatable {
        /// The model dropped or duplicated placeholders, so restoring would lose content.
        case placeholdersChanged(missing: [Int], duplicated: [Int])
    }

    /// Terms the user never wants changed (product names, SKUs, team jargon).
    public var glossary: [String]

    public init(glossary: [String] = []) {
        self.glossary = glossary
    }

    // Order matters. Text that already looks like a placeholder goes first, while nothing of
    // ours is in the text to confuse it with; then code, so nothing inside code is matched by
    // later patterns.
    private static let builtInPatterns: [String] = [
        placeholderPattern,
        "```[\\s\\S]*?```",  // code block
        "`[^`\\n]+`",  // inline code
        "<[^>\\s]+>",  // Slack raw tokens: <@U123>, <#C123|name>, <https://...|label>
        "https?://[^\\s<>]*[^\\s<>.,!?;:)\\]'\"]",  // URLs, minus trailing punctuation
        "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}",  // emails
        "(?<![\\w@])@\\w(?:[\\w.\\-]*\\w)?",  // @mentions, minus trailing punctuation
        "(?<![\\w&])#[\\w\\-]+",  // #channels
        ":[a-z0-9_+\\-]+:",  // :emoji:
    ]

    public func mask(_ input: String) -> Masked {
        var text = input
        var originals: [String] = []

        var patterns = Self.builtInPatterns
        // Longest glossary terms first so "Opentaps CRM" wins over "Opentaps".
        for term in glossary.map({ $0.trimmingCharacters(in: .whitespaces) })
            .filter({ !$0.isEmpty })
            .sorted(by: { $0.count > $1.count })
        {
            patterns.append("(?<!\\w)" + NSRegularExpression.escapedPattern(for: term) + "(?!\\w)")
        }

        for pattern in patterns {
            // Built in, or a glossary term escaped above: a pattern that fails is a bug here.
            guard let regex = try? NSRegularExpression(pattern: pattern, options: []) else {
                assertionFailure("Protector pattern \(pattern) doesn't compile"); continue
            }
            let ns = text as NSString
            let matches = regex.matches(in: text, range: NSRange(location: 0, length: ns.length))
            guard !matches.isEmpty else { continue }

            // Placeholders from earlier passes stay as they are; one a later pattern swallows
            // whole (inside code, say) goes into that piece and comes back out with it.
            let before = originals.count
            var rebuilt = ""
            var cursor = 0
            for match in matches {
                let range = match.range
                let found = ns.substring(with: range)
                if Self.isOurPlaceholder(found, count: before) { continue }
                rebuilt += ns.substring(with: NSRange(location: cursor, length: range.location - cursor))
                originals.append(found)
                rebuilt += Self.placeholder(originals.count)
                cursor = range.location + range.length
            }
            rebuilt += ns.substring(from: cursor)
            text = rebuilt
        }
        return Masked(text: text, originals: originals)
    }

    public func restore(_ output: String, from masked: Masked) throws -> String {
        guard !masked.originals.isEmpty else { return output }

        var missing: [Int] = []
        var duplicated: [Int] = []
        // Only the placeholders the model was shown: one inside another piece comes back with it.
        for index in 1...masked.originals.count where masked.text.contains(Self.placeholder(index)) {
            let count = output.components(separatedBy: Self.placeholder(index)).count - 1
            if count == 0 { missing.append(index) }
            if count > 1 { duplicated.append(index) }
        }
        guard missing.isEmpty, duplicated.isEmpty else {
            throw RestoreError.placeholdersChanged(missing: missing, duplicated: duplicated)
        }
        return restoringWhatsThere(output, from: masked)
    }

    /// Puts back every placeholder that's in `output`, whole, and leaves the rest: for text
    /// still streaming, which may not have reached them all yet.
    public func restoringWhatsThere(_ output: String, from masked: Masked) -> String {
        // Highest index first so {{1}} never clobbers part of {{12}}.
        var text = output
        for index in stride(from: masked.originals.count, through: 1, by: -1) {
            text = text.replacingOccurrences(of: Self.placeholder(index), with: masked.originals[index - 1])
        }
        return text
    }

    /// What the model sees in place of the `n`th protected piece (counting from 1).
    public static func placeholder(_ n: Int) -> String { "{{\(n)}}" }
    /// Matches any placeholder, for code that must treat one as a single word.
    public static let placeholderPattern = #"\{\{\d+\}\}"#

    private static func isOurPlaceholder(_ s: String, count: Int) -> Bool {
        guard s.hasPrefix("{{"), s.hasSuffix("}}"), let n = Int(s.dropFirst(2).dropLast(2)) else { return false }
        return n >= 1 && n <= count
    }
}
