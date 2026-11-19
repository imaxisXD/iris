import Foundation
import Testing
@testable import IrisCore

@Suite struct ProblemNoteTests {
    /// The bug: the same failure read "came back empty. one more go?" in the panel and
    /// "Iris came back with nothing." by your text. One source now.
    @Test func everyCheckFailureSaysWhatToDoNext() {
        let all: [RewriteError] = [.emptyInput, .tooLong(limit: 6000), .offTask, .timedOut, .emptyOutput, .lostProtectedText]
        for error in all {
            let note = ProblemNote(error)
            #expect(!note.lead.isEmpty && note.lead == note.lead.lowercased())
            #expect(note.detail.hasSuffix("."))
            // Every detail is an instruction: try again, or what to do instead.
            #expect(note.detail.contains("Try again") || note.detail.contains("then"))
        }
    }

    @Test func onlyRetriesThatCanWorkOfferTryAgain() {
        #expect(!ProblemNote(.emptyInput).canRetry)
        #expect(!ProblemNote(.tooLong(limit: 6000)).canRetry)
        #expect(ProblemNote(.timedOut).canRetry)
        #expect(!ProblemNote(ModelFailure.noRoom(needed: "6.2 GB", free: "3.1 GB")).canRetry)
        #expect(ProblemNote(ModelFailure.offline).canRetry)
        #expect(!ProblemNote(ModelFailure.notDownloaded).canRetry)
    }

    /// The bug: offline showed "The operation couldn't be completed. (NSURLErrorDomain error -1009.)".
    /// A failure carries no system text now, and none of Iris's own words read like it.
    @Test func modelFailuresNeverShowSystemWords() {
        let failures: [ModelFailure] = [.noRoom(needed: "6.2 GB", free: "3.1 GB"), .offline, .notDownloaded, .other]
        for words in failures.flatMap({ [$0.sentence, ProblemNote($0).detail] }) + [ProblemNote.unknown.detail] {
            #expect(!words.lowercased().contains("error"))
        }
    }

    /// The lead and the detail never say the same thing twice ("no internet. No internet.").
    @Test func theDetailNeverRepeatsTheLead() {
        let notes = [RewriteError.emptyInput, .tooLong(limit: 6000), .offTask, .timedOut, .emptyOutput, .lostProtectedText].map(ProblemNote.init)
            + [ModelFailure.noRoom(needed: "6.2 GB", free: "3.1 GB"), .offline, .notDownloaded, .other].map(ProblemNote.init)
        for note in notes { #expect(!note.detail.lowercased().hasPrefix(note.lead)) }
    }

    @Test func theLongestDetailStaysShortEnoughToRead() {
        let longest = ([RewriteError.emptyInput, .tooLong(limit: 6000), .offTask, .timedOut, .emptyOutput, .lostProtectedText]
            .map { ProblemNote($0).detail } + [ProblemNote(ModelFailure.noRoom(needed: "6.2 GB", free: "3.1 GB")).detail])
            .map(\.count).max() ?? 0
        #expect(longest <= 100)
    }

    /// The bug: "hi" that the model couldn't read as a message showed "too short" in the panel,
    /// but Quick Fix went by the general rule (another try could work), opened the panel to
    /// try again, and the panel then said it couldn't. Both read this now.
    @Test func aDraftTooShortToReadIsCircledNotRetried() {
        let short = ProblemNote(.offTask, draft: "  hi ")
        #expect(short.circled && !short.canRetry)
        #expect(short.detail.contains("\u{201C}hi\u{201D}"))
        #expect(ProblemNote(.offTask, draft: "k").note.hasPrefix("just one letter"))
        // A real message the model talked about instead: another try can work.
        let long = ProblemNote(.offTask, draft: "can you check the order status for the customer today")
        #expect(!long.circled && long.canRetry)
        // Only the model talking about the text counts; a timeout on "hi" can still be retried.
        #expect(ProblemNote(.timedOut, draft: "hi").canRetry)
    }

    @Test func modelFailuresSayTheSameThingEverywhere() {
        for failure in [ModelFailure.noRoom(needed: "6.2 GB", free: "3.1 GB"), .offline, .notDownloaded, .other] {
            #expect(ProblemNote(failure).detail == failure.nextStep)
            #expect(ProblemNote(failure).canRetry == failure.canRetry)
        }
    }
}

