import Testing
@testable import IrisCore

/// Properties of the diff the panel draws (pink for added words, struck for removed ones), tried
/// on hundreds of made-up edits rather than the few someone thought of: what's kept plus what's
/// added must be exactly the new text, and what's kept plus what's removed exactly the original,
/// or the panel would show words that aren't there or drop ones that are.
struct WordDiffPropertyTests {
    /// A small seeded generator, so a failure names an input that fails again.
    struct Seeded {
        var state: UInt64
        mutating func next(_ bound: Int) -> Int {
            state = state &* 6364136223846793005 &+ 1442695040888963407
            return Int((state >> 33) % UInt64(bound))
        }
    }

    static let words = ["hey", "i", "I", "have", "checked", "the", "PR", "and", "its", "it's", "look", "looks", "fine", "but",
                        "one", "thing", "need", "needs", "to", "change", "before", "merge", "@lead", "don’t", "café", "42"]
    static let marks = [",", ".", "!", "?", ":", " - ", "\n"]

    static func sentence(_ rng: inout Seeded) -> String {
        var out = ""
        for i in 0..<(1 + rng.next(14)) {
            if i > 0 { out += rng.next(6) == 0 ? "  " : " " }
            out += words[rng.next(words.count)]
            if rng.next(5) == 0 { out += marks[rng.next(marks.count)] }
        }
        return out
    }

    /// An edit like a model's: some words kept, some dropped, some swapped, some added.
    static func edit(_ text: String, _ rng: inout Seeded) -> String {
        var tokens = WordDiff.tokenize(text)
        for _ in 0..<(1 + rng.next(4)) where !tokens.isEmpty {
            let at = rng.next(tokens.count)
            switch rng.next(3) {
            case 0: tokens.remove(at: at)
            case 1: tokens[at] = words[rng.next(words.count)]
            default: tokens.insert(words[rng.next(words.count)] + " ", at: at)
            }
        }
        return tokens.joined()
    }

    static func cases() -> [(String, String)] {
        var rng = Seeded(state: 20261117)
        return (0..<400).map { _ in
            let original = sentence(&rng)
            return (original, rng.next(8) == 0 ? original : edit(original, &rng))
        }
    }

    @Test func keptPlusAddedIsTheNewText() {
        for (original, fixed) in Self.cases() {
            let rebuilt = WordDiff.diff(original, fixed).map { segment -> String in
                switch segment {
                case .same(let s), .added(let s): s
                case .removed: ""
                }
            }.joined()
            #expect(rebuilt == fixed, "diff(\(original.debugDescription), \(fixed.debugDescription))")
        }
    }

    @Test func keptPlusRemovedIsTheOriginal() {
        for (original, fixed) in Self.cases() {
            let rebuilt = WordDiff.diff(original, fixed).map { segment -> String in
                switch segment {
                case .same(let s), .removed(let s): s
                case .added: ""
                }
            }.joined()
            #expect(rebuilt == original, "diff(\(original.debugDescription), \(fixed.debugDescription))")
        }
    }

    @Test func noChangeIsAllKept() {
        for (original, _) in Self.cases() {
            #expect(WordDiff.diff(original, original).allSatisfy { if case .same = $0 { true } else { false } })
        }
    }
}
