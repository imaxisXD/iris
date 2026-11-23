import Foundation

/// One wording per problem, so the panel's red-pen note, the message by your text and setup
/// never say the same failure two ways. Each says what happened in Iris's voice and what to
/// do next in a plain sentence, and whether another try can help (only then is there a
/// Try again). The system's own words never reach the screen.
public struct ProblemNote: Equatable, Sendable {
    /// Iris's note in red brush in the panel, lowercase, first person.
    public let note: String
    /// The short brush lead of the message by your text.
    public let lead: String
    /// What to do next, in a plain sentence.
    public let detail: String
    /// Another attempt can work, so Try again is offered (and Quick Fix opens the panel, where
    /// it is, instead of leaving you with a message).
    public let canRetry: Bool
    /// The draft is a word or two, so the panel's pen circles it instead of quoting it.
    public let circled: Bool

    public init(note: String, lead: String, detail: String, canRetry: Bool, circled: Bool = false) {
        self.note = note
        self.lead = lead
        self.detail = detail
        self.canRetry = canRetry
        self.circled = circled
    }

    /// A check of `draft` that failed with `error`. A stray letter or a half-captured field the
    /// model couldn't read as a message is too short to rewrite: no point trying again, so Iris
    /// circles what it read and says so, in the panel and by your text alike.
    public init(_ error: RewriteError, draft: String) {
        let read = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard error == .offTask, FixGuard.wordCount(read) < 3, read.count <= 30 else { self.init(error); return }
        self.init(note: read.count == 1 ? "just one letter? give me a sentence" : "too short. give me a sentence",
                  lead: "too short to rewrite",
                  detail: "Iris read \u{201C}\(read)\u{201D}, which is too short to rewrite. Your text wasn't changed.",
                  canRetry: false, circled: true)
    }

    public init(_ error: RewriteError) {
        switch error {
        case .emptyInput:
            self.init(note: "nothing here to rewrite", lead: "nothing to fix",
                      detail: "The box is empty. Type something, then try again.", canRetry: false)
        case .tooLong(let limit):
            self.init(note: "that's a lot. select a part and call me again", lead: "that's a lot",
                      detail: "Iris checks about \(limit / 6) words at once. Select a part, then press the keys again.", canRetry: false)
        case .offTask:
            self.init(note: "couldn't make sense of this. one more go?", lead: "couldn't make sense of it",
                      detail: "Iris couldn't read this as a message and left your text alone. Try again.", canRetry: true)
        case .timedOut:
            self.init(note: "ran out of time. one more go?", lead: "ran out of time",
                      detail: "Iris stopped and left your text alone. Try again.", canRetry: true)
        case .emptyOutput:
            self.init(note: "came back empty. one more go?", lead: "came back empty",
                      detail: "Iris left your text alone. Try again.", canRetry: true)
        case .lostProtectedText:
            self.init(note: "dropped a name or link. one more go?", lead: "dropped a name or link",
                      detail: "Iris kept your text so nothing went missing. Try again.", canRetry: true)
        }
    }

    /// Getting the model failed: what to do is `ModelFailure.nextStep`, and Try again is offered
    /// when the failure says another try can work.
    public init(_ failure: ModelFailure) {
        #if os(Windows)
        let place = ("open setup from the iris icon by the clock", "this pc")
        #else
        let place = ("open setup from the menu bar", "this mac")
        #endif
        let (note, lead): (String, String) = switch failure {
        case .notDownloaded: ("i need my model first. \(place.0)", "i need my model first")
        case .noRoom: ("no room for my model on \(place.1)", "no room for my model")
        case .offline: ("no internet for my model. one more go?", "no internet")
        case .other: ("couldn't finish that. one more go?", "couldn't get the model")
        }
        self.init(note: note, lead: lead, detail: failure.nextStep, canRetry: failure.canRetry)
    }

    /// Anything else: no system words on screen, just what to do.
    public static let unknown = ProblemNote(note: "couldn't finish that. one more go?", lead: "couldn't fix",
                                            detail: "Something got in the way and Iris left your text alone. Try again in a moment.",
                                            canRetry: true)
}
