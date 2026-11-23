import Foundation

/// Word-level diff between the user's text and the model's text, used to show
/// exactly what changed and to judge whether a "fix" turned into a rewrite.
public enum WordDiff {
    public enum Segment: Equatable, Sendable {
        case same(String)
        case removed(String)
        case added(String)
    }

    private static let maxDPTableCells = 4_000_000

    private static let tokenRegex = try! NSRegularExpression(
        pattern: "\\s+|[\\p{L}\\p{N}_]+(?:['’][\\p{L}\\p{N}_]+)*|" + Protector.placeholderPattern + "|[^\\p{L}\\p{N}_\\s]"
    )

    /// Splits into words (keeping apostrophes: "it's", "don’t"), whitespace runs,
    /// and single punctuation marks, so "its" -> "it's" is one word change.
    public static func tokenize(_ text: String) -> [String] {
        let ns = text as NSString
        return tokenRegex.matches(in: text, range: NSRange(location: 0, length: ns.length))
            .map { ns.substring(with: $0.range) }
    }

    public static func diff(_ old: String, _ new: String) -> [Segment] {
        if old == new {
            return old.isEmpty ? [] : [.same(old)]
        }
        let a = tokenize(old)
        let b = tokenize(new)

        // Strip common prefix and suffix before running DP so minor edits on long drafts
        // avoid allocating and scanning an O(N*M) table over unchanged text.
        var prefixLen = 0
        let minLen = min(a.count, b.count)
        while prefixLen < minLen && a[prefixLen] == b[prefixLen] {
            prefixLen += 1
        }
        var suffixLen = 0
        while suffixLen < (minLen - prefixLen)
            && a[a.count - 1 - suffixLen] == b[b.count - 1 - suffixLen]
        {
            suffixLen += 1
        }

        let aMidCount = a.count - prefixLen - suffixLen
        let bMidCount = b.count - prefixLen - suffixLen
        var raw: [Segment] = []
        raw.reserveCapacity(a.count + b.count)

        for k in 0 ..< prefixLen {
            raw.append(.same(a[k]))
        }

        if aMidCount == 0 {
            for k in prefixLen ..< (prefixLen + bMidCount) {
                raw.append(.added(b[k]))
            }
        } else if bMidCount == 0 {
            for k in prefixLen ..< (prefixLen + aMidCount) {
                raw.append(.removed(a[k]))
            }
        } else if (aMidCount + 1) > Self.maxDPTableCells / (bMidCount + 1) {
            // Very large full-text rewrite: skip allocating a huge O(N*M) table and show the
            // differing span as one replacement block.
            for k in prefixLen ..< (prefixLen + aMidCount) { raw.append(.removed(a[k])) }
            for k in prefixLen ..< (prefixLen + bMidCount) { raw.append(.added(b[k])) }
        } else {
            // Flat contiguous Int32 LCS table (instead of array-of-arrays) for cache locality.
            let strideM = bMidCount + 1
            var lcs = [Int32](repeating: 0, count: (aMidCount + 1) * strideM)
            for i in stride(from: aMidCount - 1, through: 0, by: -1) {
                let rowOff = i * strideM
                let nextRowOff = rowOff + strideM
                let ai = a[prefixLen + i]
                for j in stride(from: bMidCount - 1, through: 0, by: -1) {
                    if ai == b[prefixLen + j] {
                        lcs[rowOff + j] = lcs[nextRowOff + j + 1] + 1
                    } else {
                        let down = lcs[nextRowOff + j]
                        let right = lcs[rowOff + j + 1]
                        lcs[rowOff + j] = down >= right ? down : right
                    }
                }
            }

            var i = 0
            var j = 0
            while i < aMidCount && j < bMidCount {
                if a[prefixLen + i] == b[prefixLen + j] {
                    raw.append(.same(a[prefixLen + i])); i += 1; j += 1
                } else if lcs[(i + 1) * strideM + j] >= lcs[i * strideM + j + 1] {
                    raw.append(.removed(a[prefixLen + i])); i += 1
                } else {
                    raw.append(.added(b[prefixLen + j])); j += 1
                }
            }
            while i < aMidCount { raw.append(.removed(a[prefixLen + i])); i += 1 }
            while j < bMidCount { raw.append(.added(b[prefixLen + j])); j += 1 }
        }

        for k in (a.count - suffixLen) ..< a.count {
            raw.append(.same(a[k]))
        }

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
