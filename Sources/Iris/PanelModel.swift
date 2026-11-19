import Combine
import Foundation
import IrisCore
import IrisEngine
import SwiftUI

/// State behind the floating panel: which mode is selected, what the model is
/// doing, and the result for each mode (cached, so Tab back to Fix is instant).
@MainActor
final class PanelModel: ObservableObject {
    enum Phase: Equatable {
        case preparing  // model downloading or loading; see `engineState`
        case generating(String)  // partial text so far
        case done(RewriteResult)
        case failed(Failure)
    }

    /// A failed check, shown the way an editor marks up a draft: a note in the margin.
    struct Failure: Equatable {
        /// Iris's margin note, lettered in brush.
        let note: String
        /// The whole story in a plain sentence, for VoiceOver.
        let detail: String
        /// Another attempt could work, so the panel offers Try again. False when it would
        /// fail the same way (no text, too long, too little to read as a message).
        let canRetry: Bool
        /// The draft is a word or two, so the pen circles it instead of quoting it.
        let circled: Bool
    }

    var failure: Failure? {
        if case .failed(let failure) = phase { return failure }
        return nil
    }

    @Published private(set) var modes: [Mode] = []
    @Published private(set) var selected = 0
    @Published private(set) var phase: Phase = .preparing
    @Published private(set) var engineState: EngineState = .idle
    @Published private(set) var original = ""
    /// The app Replace pastes into, shown beside the button on its tape.
    @Published private(set) var appName: String?
    /// That app's bundle ID, for its icon on the label.
    @Published private(set) var appBundleID: String?
    /// False when the text can't be edited where it is: the main button copies instead.
    @Published private(set) var canReplace = true
    /// Copy pressed on text that can't be edited.
    var onCopyOut: ((String) -> Void)?
    @Published private(set) var copiedFlash = false
    /// Shown briefly when "Try again" came back with the same words, so it doesn't look broken.
    @Published private(set) var retryNote: String?
    /// What clears each of those, so a newer one is never cut short by an older timer.
    private var copiedFlashTask: Task<Void, Never>?
    private var retryNoteTask: Task<Void, Never>?

    /// The draft is mostly another language (e.g. romanized Hindi), so the answer is in English
    /// and there's no diff to show.
    var translated: Bool { currentResult?.translated ?? false }

    var onReplace: ((String) -> Void)?
    var onClose: (() -> Void)?

    private var results: [String: RewriteResult] = [:]
    /// How this panel checks, read from Settings when it opened.
    private var setup = CheckSetup.current

    /// Results from earlier panels, so re-checking the same text in the same tone is instant.
    /// Keyed by everything an answer depends on (`CheckSetup.key`).
    private static var memo: [String: RewriteResult] = [:]
    private static var memoOrder: [String] = []
    private func memoKey(_ mode: Mode) -> String { setup.key(for: original, mode: mode) }
    private func remember(_ result: RewriteResult) {
        let key = memoKey(result.mode)
        Self.memo[key] = result
        Self.memoOrder.removeAll { $0 == key }
        Self.memoOrder.append(key)
        while Self.memoOrder.count > 32 { Self.memo[Self.memoOrder.removeFirst()] = nil }
    }
    private var task: Task<Void, Never>?
    private let settings = AppSettings.shared
    private var engineWatch: AnyCancellable?

    init() {
        engineWatch = EngineMonitor.shared.$state.sink { [weak self] state in self?.engineStateChanged(state) }
    }

    var currentMode: Mode { modes.indices.contains(selected) ? modes[selected] : .fix }

    var currentResult: RewriteResult? {
        if case .done(let result) = phase { return result }
        return nil
    }

    /// Called when Fix finds nothing and the user prefers a short note over the panel.
    var onClean: (() -> Void)?

    /// `answer`: one already in hand for this text (Quick Fix's, when it wants a person to
    /// look), so the panel shows it instead of asking the model again.
    func start(text: String, modeID: String, answer: RewriteResult? = nil, appName: String? = nil,
               appBundleID: String? = nil, canReplace: Bool = true) {
        original = text
        self.appName = appName
        self.appBundleID = appBundleID
        self.canReplace = canReplace
        setup = .current
        results = [:]
        clearRetryNote()
        clearCopiedFlash()
        modes = settings.modes
        selected = modes.firstIndex(where: { $0.id == modeID }) ?? 0
        if let answer, answer.original == text { remember(answer) }
        run()
    }

    func select(_ index: Int) {
        guard modes.indices.contains(index), index != selected else { return }
        selected = index
        clearRetryNote()
        if let cached = results[currentMode.id] {
            task?.cancel()
            phase = .done(cached)
        } else {
            run()
        }
    }

    func next() { select((selected + 1) % max(modes.count, 1)) }
    func previous() { select((selected - 1 + modes.count) % max(modes.count, 1)) }

    /// "Try again": always a fresh generation, never the cached answer.
    func regenerate() {
        // No Try again where another attempt fails the same way (no text, too long).
        if let failure, !failure.canRetry { return }
        let previous = currentResult?.text
        results[currentMode.id] = nil
        Self.memo[memoKey(currentMode)] = nil
        clearRetryNote()
        run(retry: true, previous: previous)
    }

    /// What the bottom row shows, from `PanelFooter`'s rules.
    var footer: PanelFooter {
        let stage: PanelFooter.Stage
        switch phase {
        case .preparing, .generating: stage = .working(fix: currentMode.isFix)
        case .done(let result): stage = .done(fix: result.mode.isFix, verdict: result.verdict)
        case .failed(let failure): stage = .failed(canRetry: failure.canRetry)
        }
        return PanelFooter(stage: stage, appName: appName, canReplace: canReplace, translated: translated,
                           copied: copiedFlash, retryNote: retryNote, toneCount: modes.count)
    }

    /// The footer's ↵ button, which Return presses too. (Try again has its own key, ⌘R.)
    func accept() {
        guard let result = currentResult else { return }
        switch footer.action {
        case .done: onClose?()
        case .replace: onReplace?(result.text)
        case .copy: onCopyOut?(result.text)
        case .tryAgain, .checking, .writing, .none: break
        }
    }

    func copy() {
        guard let result = currentResult else { return }
        TextBridge.copyToClipboard(result.text)
        copiedFlash = true
        copiedFlashTask?.cancel()
        copiedFlashTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(1.2))
            guard !Task.isCancelled else { return }
            self?.copiedFlash = false
        }
    }

    private func clearCopiedFlash() {
        copiedFlashTask?.cancel()
        copiedFlash = false
    }

    private func showRetryNote(_ note: String) {
        retryNote = note
        retryNoteTask?.cancel()
        retryNoteTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(3))
            guard !Task.isCancelled else { return }
            self?.retryNote = nil
        }
    }

    private func clearRetryNote() {
        retryNoteTask?.cancel()
        retryNote = nil
    }

    func cancel() {
        task?.cancel()
        task = nil
    }

    /// Puts the panel into a fixed state for design snapshots (no model involved), with the
    /// tones every new Mac starts with, so the pictures don't depend on this Mac's settings.
    func preview(original: String, result: RewriteResult?, selected: Int = 0, engine: EngineState, streaming: String? = nil,
                 error: Error? = nil, appName: String? = "Slack", appBundleID: String? = "com.tinyspeck.slackmacgap",
                 canReplace: Bool = true, copied: Bool = false, retryNote: String? = nil) {
        previewing = true
        copiedFlash = copied
        self.retryNote = retryNote
        self.original = original
        self.appName = appName
        self.appBundleID = appBundleID
        self.canReplace = canReplace
        modes = ToneList.newMac.shown
        self.selected = selected
        engineState = engine
        if let streaming { phase = .generating(streaming) } else { phase = result.map { .done($0) } ?? .preparing }
        if let error { phase = failure(error) }
    }

    private func run(retry: Bool = false, previous: String? = nil) {
        task?.cancel()
        let mode = currentMode
        let text = original
        if !retry, let cached = Self.memo[memoKey(mode)] {
            results[mode.id] = cached
            phase = .done(cached)
            // The same rule as a fresh answer, a turn later: the panel is still being stood
            // up while `start` runs, and can only step aside once it's there.
            if closesWhenClean(cached) {
                task = Task { [weak self] in
                    guard !Task.isCancelled else { return }
                    self?.onClean?()
                }
            }
            return
        }
        let checker = setup.checker
        phase = checker.usesModel && !isEngineReady ? .preparing : .generating("")

        task = Task { [weak self] in
            do {
                for try await event in checker.run(text, mode: mode, retry: retry) {
                    guard let self, !Task.isCancelled else { return }
                    switch event {
                    case .partial(let partial):
                        self.phase = .generating(partial)
                    case .finished(let result):
                        self.results[mode.id] = result
                        self.remember(result)
                        self.prewarmNext(after: mode, checker: checker)
                        guard self.currentMode.id == mode.id else { return }
                        self.phase = .done(result)
                        if retry, result.text == previous {
                            self.showRetryNote(mode.isFix ? "Same fix again, Iris is confident in it" : "Came out the same, try once more")
                        }
                        if self.closesWhenClean(result) { self.onClean?() }
                    }
                }
            } catch {
                // Cancelled by the panel itself (another check, another tone, closing): nothing
                // to say. A cancellation that reaches a check still wanted came from the model
                // (its load was replaced or it was deleted meanwhile), and is a failure like any
                // other: say so with Try again, never wait forever.
                guard let self, !Task.isCancelled, self.currentMode.id == mode.id else { return }
                self.phase = self.failure(error)
            }
        }
    }

    /// Fix found nothing to change and Settings asks for a short note instead of the panel.
    private func closesWhenClean(_ result: RewriteResult) -> Bool {
        result.verdict == .unchanged && !settings.showPanelWhenClean
    }

    private func failure(_ error: Error) -> Phase {
        // One wording per problem, shared with the message by your text (`ProblemNote`).
        let problem = ProblemNote(error, draft: original)
        return .failed(Failure(note: problem.note, detail: problem.detail, canRetry: problem.canRetry, circled: problem.circled))
    }

    /// While the user reads this result, prefill the next tone's fixed prompt so Tab is quick.
    private func prewarmNext(after mode: Mode, checker: any Checker) {
        guard let index = modes.firstIndex(where: { $0.id == mode.id }), modes.count > 1 else { return }
        let next = modes[(index + 1) % modes.count]
        Task.detached(priority: .utility) { await checker.prewarm(mode: next) }
    }

    private var isEngineReady: Bool { engineState.isReady }

    private var previewing = false

    private func engineStateChanged(_ state: EngineState) {
        guard !previewing else { return }
        engineState = state
        if state.isReady, phase == .preparing { phase = .generating("") }
    }
}
