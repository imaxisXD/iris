import Foundation

/// A word the spell checker doesn't know, with its guesses, handed to the model alongside a
/// Fix ("then favourism" → favours? favouritism?). The model decides: small models read past
/// a typo in a short fragment, but they also know a name or a Hindi word when they see one,
/// which the spell checker doesn't. Applying the checker's corrections directly was tried
/// and turned "creds" into "crews" and "simlink" into "smiling".
public struct SpellingHint: Equatable, Sendable {
    public let word: String
    public let guesses: [String]

    public init(_ word: String, _ guesses: [String]) {
        self.word = word
        self.guesses = guesses
    }

    /// At most this many hints per message, so a pasted list of names can't swamp the prompt.
    public static let limit = 8

    /// Whether a flagged word is worth asking about. Skipped: a capitalized word inside a
    /// sentence (a name), words with digits or inner capitals (codes, CamelCase), and common
    /// romanized Hindi words, for which the checker suggests English ("kar" → car).
    public static func isWorthAsking(_ word: String, atSentenceStart: Bool) -> Bool {
        guard word.count >= 3, word.allSatisfy({ $0.isLetter || $0 == "'" }) else { return false }
        if word.dropFirst().contains(where: \.isUppercase) { return false }
        if word.first?.isUppercase == true, !atSentenceStart { return false }
        return !hindiWords.contains(word.lowercased())
    }

    /// True when only spaces, quotes or brackets follow the last sentence end (or nothing at
    /// all comes before), i.e. a word placed here starts a sentence.
    public static func startsSentence(after prefix: String) -> Bool {
        let skipped = CharacterSet.whitespacesAndNewlines.union(CharacterSet(charactersIn: "\"'“‘("))
        // What follows the last real character: a line break in it starts a sentence too.
        let tail = prefix.unicodeScalars.reversed().prefix(while: skipped.contains)
        guard tail.count < prefix.unicodeScalars.count else { return true }
        if tail.contains(where: CharacterSet.newlines.contains) { return true }
        let last = prefix.unicodeScalars[prefix.unicodeScalars.index(prefix.unicodeScalars.endIndex, offsetBy: -tail.count - 1)]
        return ".!?".unicodeScalars.contains(last)
    }

    /// Common romanized Hindi words that aren't also English words.
    static let hindiWords: Set<String> = [
        "hai", "hain", "tha", "thi", "kar", "karo", "karna", "karu", "karenge", "kiya", "kya", "kyu", "kyun",
        "nahi", "nahin", "haan", "accha", "achha", "acha", "theek", "thik", "bhai", "yaar", "thoda", "jaldi",
        "abhi", "aaj", "kal", "pehle", "baad", "mein", "bhi", "toh", "aur", "raha", "rahi", "rahe", "gaya",
        "gayi", "diya", "liya", "dena", "lena", "dekh", "dekho", "dekhte", "bata", "batao", "bhej", "bhejo",
        "chahiye", "jayega", "jayegi", "hoga", "hogi", "kuch", "sab", "kaise", "kaha", "kahan", "matlab",
        "wala", "wali", "woh", "yeh", "karke", "lekin", "phir", "sahi", "bas", "chalo", "arre", "uska",
        "uski", "unka", "jyada", "zyada", "bohot", "bahut", "kaam", "wahi", "yahi", "mujhe", "tujhe",
    ]
}
