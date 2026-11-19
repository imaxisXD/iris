import Foundation

/// Word-level diff between the user's text and the model's text, used to show
/// exactly what changed and to judge whether a "fix" turned into a rewrite.
public enum WordDiff {
    public enum Segment: Equatable, Sendable {
        case same(String)
        case removed(String)
        case added(String)
    }

    /// Splits into words (keeping apostrophes: "it's", "don’t"), whitespace runs,
    /// and single punctuation marks, so "its" -> "it's" is one word change.
    public static func tokenize(_ text: String) -> [String] {
        let pattern = "\\s+|[\\p{L}\\p{N}_]+(?:['’][\\p{L}\\p{N}_]+)*|" + Protector.placeholderPattern + "|[^\\p{L}\\p{N}_\\s]"
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return [text] }
        let ns = text as NSString
        return regex.matches(in: text, range: NSRange(location: 0, length: ns.length))
            .map { ns.substring(with: $0.range) }
    }

    public static func diff(_ old: String, _ new: String) -> [Segment] {
        let a = tokenize(old)
        let b = tokenize(new)
        let n = a.count
        let m = b.count

        // Classic LCS table. Chat messages are short, so O(n*m) is fine.
        var lcs = Array(repeating: Array(repeating: 0, count: m + 1), count: n + 1)
        if n > 0 && m > 0 {
            for i in stride(from: n - 1, through: 0, by: -1) {
                for j in stride(from: m - 1, through: 0, by: -1) {
                    lcs[i][j] = a[i] == b[j] ? lcs[i + 1][j + 1] + 1 : max(lcs[i + 1][j], lcs[i][j + 1])
                }
            }
        }

        var raw: [Segment] = []
        var i = 0
        var j = 0
        while i < n && j < m {
            if a[i] == b[j] {
                raw.append(.same(a[i])); i += 1; j += 1
            } else if lcs[i + 1][j] >= lcs[i][j + 1] {
                raw.append(.removed(a[i])); i += 1
            } else {
                raw.append(.added(b[j])); j += 1
            }
        }
        while i < n { raw.append(.removed(a[i])); i += 1 }
        while j < m { raw.append(.added(b[j])); j += 1 }

        return merge(absorbingSpaces(raw))
    }

    /// A lone unchanged space between two changes reads better as part of the change:
    /// "~its look~ it looks" instead of "~its~ it ~look~ looks".
    private static func absorbingSpaces(_ segments: [Segment]) -> [Segment] {
        var out: [Segment] = []
        for (index, segment) in segments.enumerated() {
            if case .same(let s) = segment, s.allSatisfy(\.isWhitespace),
                index > 0, index < segments.count - 1,
                isChange(segments[index - 1]), isChange(segments[index + 1])
            {
                out.append(.removed(s))
                out.append(.added(s))
            } else {
                out.append(segment)
            }
        }
        // Group removals before additions within each run of changes.
        var grouped: [Segment] = []
        var removed: [Segment] = []
        var added: [Segment] = []
        func flush() {
            grouped += removed + added
            removed.removeAll(); added.removeAll()
        }
        for segment in out {
            switch segment {
            case .removed: removed.append(segment)
            case .added: added.append(segment)
            case .same: flush(); grouped.append(segment)
            }
        }
        flush()
        return grouped
    }

    private static func merge(_ segments: [Segment]) -> [Segment] {
        var out: [Segment] = []
        for segment in segments {
            switch (out.last, segment) {
            case (.same(let x)?, .same(let y)): out[out.count - 1] = .same(x + y)
            case (.removed(let x)?, .removed(let y)): out[out.count - 1] = .removed(x + y)
            case (.added(let x)?, .added(let y)): out[out.count - 1] = .added(x + y)
            default: out.append(segment)
            }
        }
        return out
    }

    private static func isChange(_ s: Segment) -> Bool {
        if case .same = s { return false }
        return true
    }
}
