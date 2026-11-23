// Copyright © 2026 Apple Inc.

import Foundation
import MLX

enum MTPInitializationError: Error, LocalizedError {
    // Only emitted by the initial staging probe, before target prefill.
    case unsupportedSpeculativeCache

    var errorDescription: String? {
        switch self {
        case .unsupportedSpeculativeCache:
            "MTP speculative decoding requires a stageable main KV cache."
        }
    }
}

/// Generator of tokens using MTP (Multi-Token Prediction) speculative
/// decoding.
///
/// Parallels ``SpeculativeTokenIterator`` but for Gemma 4 - style drafters
/// that share K/V with the target model and produce K - 1 candidate tokens
/// per round in a single ``MTPDrafterModel/draftBlock(target:lastToken:lastHidden:sharedKV:positionDeltas:queryOffset:blockSize:sampler:)`` call (rather
/// than K sequential single-token calls). Every per-round input —
/// `lastToken`, `lastHidden`, `sharedKV`, `positionIds` — is threaded as a
/// method argument, with the target's last hidden state and per-`layer_type`
/// shared K/V extracted from the ``LMOutput/State`` emitted by the target on
/// the previous main-model call.
/// If the drafter needs its own KV cache (Qwen MTP), that cache is owned by
/// this iterator, prefilled over the shifted prompt, and reconciled against
/// accepted target tokens after every verify pass; it is never stored on the
/// shared drafter model.
///
/// The iterator pre-populates each main-model call's incoming `state` with
/// ``mtpEmitFlagKey`` set to `true`, opting the target into populating
/// ``mtpLastHiddenStatesKey`` and ``mtpSharedKVStatesKey`` on its returned
/// ``LMOutput/state``. If the target ever returns nil or partial state
/// (for example when a shared-KV drafter can no longer read regular K/V), the
/// iterator transparently switches into a
/// single-token "passthrough" mode for the remainder of generation —
/// a mid-generation capability loss never crashes or corrupts the stream.
///
/// Port of `_speculative_walk` from mlx-vlm/generate.py at SHA `d49d428`,
/// with no-mutation-during-eval idioms (state is threaded through method
/// args; drafter holds no target-derived state — the target and optional
/// per-stream drafter cache are passed as parameters to `draftBlock(...)` so
/// drafter instances are safe to share across iterators).
public struct MTPSpeculativeTokenIterator: TokenIteratorProtocol {

    var y: LMInput.Text

    let mainModel: any LanguageModel
    let drafter: any MTPDrafterModel

    var mainState: LMOutput.State?
    let mainCacheStorage: KVCacheStorage
    var mainCache: [KVCache] { mainCacheStorage.cache }
    var kvCachePlan: KVCachePlan { mainCacheStorage.plan }
    var drafterState: MTPDrafterState?
    private var continuationToResume: MTPDrafterContinuation?
    private var finalizedContinuation: MTPDrafterContinuation?

    /// Hidden state of the final token currently represented by the target
    /// cache. This is the architecture-neutral seam a resumable drafter uses
    /// to bridge a later prompt suffix.
    private var targetBoundaryHidden: MLXArray?
    private var lastVerifyHidden: MLXArray?
    private var lastVerifyStart = 0
    private var lastVerifyAccepted = 0

    var processor: LogitProcessor?
    let sampler: LogitSampler

    /// Sampler handed to ``MTPDrafterModel/draftBlock``: the base sampler,
    /// wrapped to mask suppressed token IDs when the target has any.
    /// `draftBlock` receives no ``LogitProcessor``, and an unsuppressed
    /// draft of a suppressed token would always be rejected by the
    /// verifier, wasting draft slots.
    let draftSampler: LogitSampler

    public var tokenCount: Int { telemetry.emittedTokenCount }
    public let maxTokens: Int?
    /// Total tokens proposed per round (`blockSize - 1` drafted, plus the
    /// bonus token from the previous verify). Mirrors mlx-vlm's
    /// `draft_block_size` parameter.
    public let blockSize: Int

    /// Iris: optional prompt-lookup drafting (PLD) source tokens (the user's draft).
    /// When greedy with a stateless drafter, matching n-grams propose up to
    /// `promptLookupMaxDraft` tokens in microseconds with zero drafter GPU calls,
    /// falling back to the MTP drafter whenever the model diverges from the draft.
    private let promptLookupTokens: [Int]
    private let promptLookupMaxDraft: Int
    private let maximumWindowBlockSize: Int
    private var emittedHistory = [Int]()
    private var emittedHistoryCount = 0
    private var lookupCursor = 0
    private var lookupMissStreak = 0
    private var lookupCooldown = 0
    private var lookupLastAccepted = 0

    private var pendingTokens = [Int]()
    private var pendingIndex = 0
    /// Number of pending tokens already represented by `mainCache`. The last
    /// verifier sample is not committed until a later forward pass.
    private var committedPendingTokenCount = 0

    /// Set to `true` when the iterator detects that the target can no
    /// longer emit drafter state (typically due to KV cache quantization
    /// converting `Gemma4SharedKVState.regular` to `.quantized`). Once set,
    /// `next()` runs single-token generation against the main model only —
    /// no further `speculateRound` calls. Sticky: never reverts to `false`.
    private var passthrough = false
    private var passthroughLoggedOnce = false

    /// Verify-position index in the prior round's emitted hidden that
    /// produced the newly-accepted bonus's logit prediction. Set at the end
    /// of each `speculateRound()`. `nil` on the first round means slice the
    /// last position (round 1's `lastHidden` has shape `[B, 1, hidden]`, so
    /// last-position == only-position == the correct slot). Round 2+ slices
    /// at this index, mirroring mlx-lm's `verify.hidden[:, accepted : accepted + 1, :]`.
    /// Mismatch (e.g. unconditional last-position) is silent: drafter still
    /// produces tokens, but they're conditioned on the wrong slot → less
    /// coherent drafts → lower acceptance, especially at higher blockSize.
    private var lastRoundAccepted: Int? = nil

    public var promptPrefillTime: TimeInterval = 0.0
    private var telemetry = SpeculativeDecodingTelemetry()
    public var speculativeDecodingTelemetry: SpeculativeDecodingTelemetry? {
        telemetry.roundCount > 0 ? telemetry : nil
    }

    public var evictedTokenCount: Int {
        KVCacheTree.leaves(in: mainCache).map(\.cache.evictedTokenCount).max() ?? 0
    }

    public var reasoningTokenCount: Int? {
        (processor as? any GenerationReasoningTokenCounting)?.generationReasoningTokenCount
    }

    public mutating func recordEmittedToken(_ token: Int) {
        processor?.didEmit(token: token)
    }

    /// Target-owned continuation state with transient MTP exchange values
    /// removed. Model-owned anchors such as VLM M-RoPE deltas remain intact.
    public var state: LMOutput.State? {
        persistentTargetState(from: mainState)
    }

    public mutating func discardGeneratedToken() {
        telemetry.discardGeneratedToken()
    }

    // Optional instrumentation used by acceptance-rate floor tests.
    // Public read-only so test cases can compute `acceptedCount /
    // proposedCount` after the stream drains.
    public private(set) var acceptedCount: Int = 0
    public private(set) var proposedCount: Int = 0

    // Reason recorded the first time sticky-passthrough engaged, or nil if
    // the iterator stayed speculative for the full stream. Surfaced through
    // ``MTPStatsCollecting`` so `generateLoopTask` can include it on the
    // emitted `.info` event.
    public private(set) var passthroughReason: String?

    public init(
        input: LMInput,
        mainModel: any LanguageModel,
        drafter: any MTPDrafterModel,
        mainCache: [KVCache]? = nil,
        parameters: GenerateParameters,
        blockSize: Int,
        components: GenerationComponents = .init(),
        promptLookupTokens: [Int]? = nil,
        promptLookupMaxDraft: Int = 16
    ) throws {
        try drafter.validateCompatibility(with: mainModel)

        let kvCachePlan = try parameters.kvCachePlan()
        try self.init(
            input: input,
            mainModel: mainModel,
            drafter: drafter,
            mainCacheStorage: KVCacheStorage(
                mainCache ?? (try mainModel.newCache(parameters: parameters)), plan: kvCachePlan),
            parameters: parameters,
            blockSize: blockSize,
            components: components,
            promptLookupTokens: promptLookupTokens,
            promptLookupMaxDraft: promptLookupMaxDraft)
    }

    package init(
        input: LMInput,
        mainModel: any LanguageModel,
        drafter: any MTPDrafterModel,
        mainCacheStorage: KVCacheStorage,
        mainState: LMOutput.State? = nil,
        mtpContinuation: MTPDrafterContinuation? = nil,
        parameters: GenerateParameters,
        blockSize: Int,
        components: GenerationComponents = .init(),
        promptLookupTokens: [Int]? = nil,
        promptLookupMaxDraft: Int = 16
    ) throws {
        try drafter.validateCompatibility(with: mainModel)

        guard blockSize >= 2 else {
            throw KVCacheError(
                message:
                    "MTPSpeculativeTokenIterator requires blockSize >= 2 (1 bonus + K-1 drafted)"
            )
        }
        let kvCachePlan = mainCacheStorage.plan
        let mainCacheStorage = try kvCachePlan.validated(mainCacheStorage)
        let mainCache = mainCacheStorage.cache
        self.y = input.text
        self.mainModel = mainModel
        self.drafter = drafter

        self.mainCacheStorage = mainCacheStorage
        self.mainState = mainState
        self.continuationToResume = mtpContinuation
        self.drafterState =
            mtpContinuation?.state
            ?? (drafter as? any StatefulMTPDrafterModel)?
            .makeState(parameters: parameters)

        self.sampler = parameters.sampler()
        try components.validate(parameters: parameters)
        self.processor = components.logitProcessor(parameters: parameters, model: mainModel)
        // Draft proposals bypass the iterator's processor chain, so suppressed
        // ids have to be masked at the drafter's own sampler: a proposal the
        // verifier will always reject is a wasted speculation slot.
        if let suppressor = makeSuppressTokensProcessor(model: mainModel) {
            self.draftSampler = SuppressTokensSampler(base: self.sampler, suppressor: suppressor)
        } else {
            self.draftSampler = self.sampler
        }

        self.maxTokens = parameters.maxTokens
        let windowMaxBlock = Self.maximumBlockSize(for: mainCache)
        self.maximumWindowBlockSize = windowMaxBlock
        self.promptLookupTokens = promptLookupTokens ?? []
        self.promptLookupMaxDraft = Swift.max(0, Swift.min(promptLookupMaxDraft, windowMaxBlock - 1))
        // A round presents `blockSize` positions at once, and a sliding layer can only show a
        // query the `maxSize` entries before it. Past that the extra drafts still decode
        // correctly -- masks are position-relative and the staged view is clamped -- but the
        // deepest ones attend over less context than the verifier gave them, so acceptance
        // stops meaning what the stats say it means. Clamp rather than trap: the block size is
        // a tuning knob, not a correctness input.
        let drafterBlockSize = Swift.min(blockSize, drafter.maximumBlockSize ?? blockSize)
        let effectiveBlockSize = Swift.min(drafterBlockSize, windowMaxBlock)
        self.blockSize = effectiveBlockSize

        // Probe by opening a round at the width rounds will actually use and discarding it,
        // rather than duplicating the leaf classification as a predicate that could drift from
        // it. Qwen's hybrid cache is the one typed exception: its target advertises a bounded
        // recurrent checkpoint and performs the round in place.
        let nativeRewindDepth =
            (mainModel as? any SpeculativeCacheRewindModel)?
            .maximumNativeTargetCacheRewind ?? 0
        let usesNativeHybridRewind =
            nativeRewindDepth >= effectiveBlockSize - 1
            && mainCache.contains { $0 is MambaCache }
            && mainCache.allSatisfy { $0.isTrimmable || $0 is MambaCache }
        if !usesNativeHybridRewind {
            guard
                let probe = self.mainCacheStorage.beginRound(
                    maximumPositions: effectiveBlockSize)
            else {
                throw MTPInitializationError.unsupportedSpeculativeCache
            }
            self.mainCacheStorage.rollback(probe)
        }

        let prefillStart = Date.timeIntervalSinceReferenceDate
        var mtpPrefill = parameters.prefill
        if drafter.requiresPromptPrefill || continuationToResume != nil {
            // The target must expose one hidden row per prompt token so a
            // private MTP cache can be filled or resumed over the suffix.
            // Until model-specific chunk aggregation is available, use the
            // reference single-forward computation for this architecture.
            mtpPrefill.stepSize = Int.max
            mtpPrefill.chunking = .unchunked
        }
        try prepare(input: input, prefill: mtpPrefill)
        self.promptPrefillTime = Date.timeIntervalSinceReferenceDate - prefillStart

        if drafter.requiresGreedySampling, parameters.temperature != 0 {
            switchToPassthrough(
                reason:
                    "Qwen MTP currently requires temperature == 0; generating without speculation"
            )
        }
    }

    static let missingSharedKVSourcesReason =
        "main model did not report which cache entries its shared K/V came from; continuing "
        + "without speculation"

    /// The tightest sliding window among the cache's layers, or `nil` when no layer slides.
    static func narrowestSlidingWindow(in cache: [KVCache]) -> Int? {
        KVCacheTree.leaves(in: cache)
            .compactMap { leaf -> Int? in
                guard case .rotating(let rotating) = leaf.kind else { return nil }
                return rotating.maxSize
            }
            .min()
    }

    /// The widest round the narrowest sliding layer can present coherently: one bonus plus a
    /// window's worth of drafts. Unbounded when no layer slides.
    static func maximumBlockSize(for cache: [KVCache]) -> Int {
        guard let narrowest = narrowestSlidingWindow(in: cache) else { return .max }
        return Swift.max(2, narrowest + 1)
    }

    /// Prefill the main model with the prompt. The drafter's own state starts
    /// empty; its first-round conditioning inputs come from the prefill's
    /// `LMOutput.state`.
    mutating func prepare(input: LMInput, prefill: PrefillParameters = .init()) throws {
        processor?.prompt(input.text.tokens)
        prefill.prefix?(mainCacheStorage.processedTokenCount)
        let inputLength = input.text.cacheSequenceLength
        let cachePositionBeforePrefill = mainCacheStorage.processedTokenCount

        let persistentState = persistentTargetState(from: mainState)
        var prefillState = persistentState ?? LMOutput.State()
        prefillState[mtpEmitFlagKey] = true
        let isGreedyStateless =
            (sampler is ArgMaxSampler)
            && (processor == nil || processor is SuppressTokensProcessor)
        prefillState[mtpSkipLogitSoftcapKey] = isGreedyStateless
        // Iris: always pass `prefillState` so targets like Gemma 4 emit drafter state directly
        // from prefill, eliminating a redundant 42-layer prime forward pass on every request.
        let incomingPrefillState: LMOutput.State? = prefillState
        switch try mainModel.prepare(
            input, cache: mainCache, state: incomingPrefillState, prefill: prefill)
        {
        case .tokens(let tokens):
            let remainingLength = tokens.cacheSequenceLength
            precondition(
                remainingLength <= inputLength,
                "Main model prepare returned more tokens than it received")
            mainCacheStorage.commitProcessedTokens(inputLength - remainingLength)
            y = tokens
            // Final prompt position not yet evaluated -- run one forward to
            // produce the bonus token AND prime drafter state.
            let result = mainModel(y[text: .newAxis], cache: mainCache, state: prefillState)
            mainCacheStorage.commitProcessedTokens(y.cacheSequenceLength)
            var logits = result.logits[0..., -1, 0...]
            logits = processor?.process(logits: logits) ?? logits
            let token = sampler.sample(logits: logits)
            processor?.didSample(token: token)
            y = .init(tokens: token)
            mainState = result.state
            // Prefill emits too, and a multi-token prefill presents a sliding layer more than its
            // window just as a verify pass does. Nothing was rejected here, so only the head
            // clamp applies -- but a snapshot that cannot be placed against the entries at all is
            // refused here for the same reason it is mid-stream, and before anything reads it.
            if !reconcileSharedKVState(
                &mainState, discarding: 0,
                lengths: mainCacheStorage.emittedLength(forLeaf:))
            {
                switchToPassthrough(reason: Self.missingSharedKVSourcesReason)
                mainState = nil
            }
            // Yield the bonus to the iterator's consumer. Without this,
            // the iterator silently starts 1 position ahead of an
            // equivalent autoregressive run, violating speculative
            // decoding's bit-exact-equivalence-to-greedy guarantee.
            let firstTok = token.item(Int.self)
            pendingTokens.append(firstTok)
            recordEmitted(firstTok)

            // the model reported per-chunk progress; the bonus forward above
            // consumed the remainder of the prompt
            let total = input.text.tokens.size
            prefill.progress?(total, total)
        case .logits(let prefillResult):
            mainCacheStorage.commitProcessedTokens(inputLength)
            // Some `prepare` implementations evaluate the final position
            // themselves and return logits directly; their `state` here may
            // or may not carry drafter state depending on whether the model
            // override threads it.
            var logits = prefillResult.logits[0..., -1, 0...]
            logits = processor?.process(logits: logits) ?? logits
            let token = sampler.sample(logits: logits)
            processor?.didSample(token: token)
            y = .init(tokens: token)
            mainState = prefillResult.state
            let placeable = reconcileSharedKVState(
                &mainState, discarding: 0,
                lengths: mainCacheStorage.emittedLength(forLeaf:))
            if !placeable {
                switchToPassthrough(reason: Self.missingSharedKVSourcesReason)
                mainState = nil
            }

            // If prefill didn't emit drafter state, do one more forward call
            // with the just-sampled bonus token to prime the state. The cost
            // is one extra token's forward pass; acceptable. Skipped once the
            // snapshot has been refused: it exists only to obtain drafter
            // state, and this stream has stopped having a use for any.
            let missingRequiredDrafterState =
                mainState?[mtpLastHiddenStatesKey] == nil
                || (drafter.requiresSharedTargetKV
                    && mainState?[mtpSharedKVStatesKey] == nil)
            if placeable, missingRequiredDrafterState {
                var primeState = mainState ?? prefillState
                primeState[mtpEmitFlagKey] = true
                let primed = mainModel(y[text: .newAxis], cache: mainCache, state: primeState)
                mainCacheStorage.commitProcessedTokens(y.cacheSequenceLength)
                mainState = primed.state
                if !reconcileSharedKVState(
                    &mainState, discarding: 0,
                    lengths: mainCacheStorage.emittedLength(forLeaf:))
                {
                    switchToPassthrough(reason: Self.missingSharedKVSourcesReason)
                    mainState = nil
                }
                // Resample bonus from this forward's logits so the chain stays
                // coherent at this position (the cache offset moves by 1, so
                // we must re-pick the bonus from the new step's logits).
                var newLogits = primed.logits[0..., -1, 0...]
                newLogits = processor?.process(logits: newLogits) ?? newLogits
                let newToken = sampler.sample(logits: newLogits)
                processor?.didSample(token: newToken)
                y = .init(tokens: newToken)
                // Yield BOTH bonuses to the consumer, in sample order.
                // `token` is the prefill-position-N sample (consumed by the
                // re-prime forward, now committed in cache); `newToken` is
                // the prefill-position-N+1 sample that becomes the input
                // to the first speculateRound.
                let t0 = token.item(Int.self)
                let t1 = newToken.item(Int.self)
                pendingTokens.append(t0)
                pendingTokens.append(t1)
                recordEmitted(t0)
                recordEmitted(t1)
                committedPendingTokenCount = 1
            } else {
                // Prefill state already carried drafter keys; the single
                // bonus is the input to the first speculateRound.
                let t0 = token.item(Int.self)
                pendingTokens.append(t0)
                recordEmitted(t0)
            }
        }

        let promptLength = input.text.tokens.dim(-1)
        if let targetHidden = mainState?[mtpLastHiddenStatesKey], targetHidden.dim(1) > 0 {
            targetBoundaryHidden = targetHidden[0..., (-1)..., 0...]
        }

        if let continuation = continuationToResume {
            defer { continuationToResume = nil }
            guard continuation.targetProcessedTokenCount == cachePositionBeforePrefill,
                let resumableDrafter = drafter as? any ResumableMTPDrafterModel,
                var currentDrafterState = drafterState,
                let suffixTargetHidden = mainState?[mtpLastHiddenStatesKey],
                suffixTargetHidden.dim(1) >= promptLength
            else {
                switchToPassthrough(
                    reason:
                        "MTP continuation is missing target suffix state or is not aligned; continuing without speculation"
                )
                return
            }

            let resumed = resumableDrafter.resumeDrafterState(
                target: mainModel,
                suffixTokens: input.text.tokens,
                suffixTargetHidden: suffixTargetHidden[0..., ..<promptLength, 0...],
                targetBoundaryHidden: continuation.targetBoundaryHidden,
                firstBonus: y.tokens,
                positionDeltas: mainState?[mtpPositionDeltasKey],
                state: &currentDrafterState,
                sampler: sampler)
            guard resumed else {
                switchToPassthrough(
                    reason:
                        "MTP drafter refused the warm continuation; continuing without speculation"
                )
                return
            }
            drafterState = currentDrafterState
        } else if drafter.requiresPromptPrefill {
            guard cachePositionBeforePrefill == 0,
                let statefulDrafter = drafter as? any StatefulMTPDrafterModel,
                var currentDrafterState = drafterState,
                let targetHidden = mainState?[mtpLastHiddenStatesKey],
                targetHidden.dim(1) >= promptLength
            else {
                switchToPassthrough(
                    reason:
                        "stateful MTP drafter cannot initialize from a warm target suffix; continuing without speculation"
                )
                return
            }
            statefulDrafter.prepareDrafterState(
                target: mainModel,
                promptTokens: input.text.tokens,
                targetHidden: targetHidden,
                firstBonus: y.tokens,
                positionDeltas: mainState?[mtpPositionDeltasKey],
                state: &currentDrafterState,
                sampler: sampler)
            drafterState = currentDrafterState
        } else if cachePositionBeforePrefill > 0,
            drafter is any StatefulMTPDrafterModel
        {
            switchToPassthrough(
                reason:
                    "stateful MTP drafter has no warm continuation; continuing without speculation"
            )
        }

        try kvCachePlan.applyAndValidate(to: mainCacheStorage)
    }

    /// Single round: draft `blockSize - 1` tokens, verify with main, accept
    /// the longest matching prefix, emit the bonus correction.
    mutating func speculateRound() {
        guard !passthrough else { return }
        // A prior all-accepted round may keep one recurrent checkpoint until
        // its pending output is drained so early finalization can rewind it.
        discardSpeculativePromptCacheCheckpoints(mainCache)

        // A speculative round can emit up to `numDraft + 1` tokens: the
        // accepted draft prefix plus the verifier's correction/bonus token.
        // Keep the whole pending buffer within the remaining output budget.
        let maxRemainingDraft: Int
        if let maxTokens {
            let remaining = maxTokens - tokenCount
            guard remaining > 0 else { return }
            maxRemainingDraft = remaining - 1
            guard maxRemainingDraft > 0 else {
                if let token = passthroughStep(maintainingDrafterState: true) {
                    pendingTokens.append(token)
                    recordEmitted(token)
                }
                return
            }
        } else {
            maxRemainingDraft = maximumWindowBlockSize - 1
        }

        // Iris: Hybrid Prompt-Lookup Drafting (PLD). When greedy with a stateless drafter
        // and not in cooldown after a typo cluster, match recent emitted tokens against the
        // user's draft to propose up to `promptLookupMaxDraft` (31 -> L=32 NAX tile) on CPU.
        let pldProposal: (drafts: [Int], matchEnd: Int)? =
            (lookupCooldown == 0 && sampler is ArgMaxSampler
                && !(drafter is any StatefulMTPDrafterModel)
                && !promptLookupTokens.isEmpty && promptLookupMaxDraft > 0)
            ? proposeFromPrompt(maxCount: Swift.min(promptLookupMaxDraft, maxRemainingDraft))
            : nil
        if lookupCooldown > 0 {
            lookupCooldown -= 1
        }

        let numDraft =
            pldProposal?.drafts.count ?? Swift.min(maxRemainingDraft, blockSize - 1)

        guard let state = mainState,
            let lastHidden = state[mtpLastHiddenStatesKey]
        else {
            switchToPassthrough(reason: "main model did not emit drafter state")
            return
        }
        let sharedKV = state[mtpSharedKVStatesKey] ?? [:]
        if drafter.requiresSharedTargetKV, sharedKV["full_attention"] == nil {
            switchToPassthrough(reason: "main model did not emit shared target K/V")
            return
        }

        // Attention-only targets verify through a staged round. Qwen's hybrid target instead
        // checkpoints its recurrent entries and rewinds them in place; the typed capability and
        // cache topology keep that exception fail-closed.
        let nativeRewindDepth =
            (mainModel as? any SpeculativeCacheRewindModel)?
            .maximumNativeTargetCacheRewind ?? 0
        let nativeHybridRewind =
            nativeRewindDepth >= numDraft
            && mainCache.contains { $0 is MambaCache }
            && mainCache.allSatisfy { $0.isTrimmable || $0 is MambaCache }
        let round =
            nativeHybridRewind
            ? nil : mainCacheStorage.beginRound(maximumPositions: numDraft + 1)
        guard nativeHybridRewind || round != nil else {
            switchToPassthrough(
                reason: "main KV cache cannot stage a speculative round; continuing without "
                    + "speculation")
            mainState = nil
            return
        }

        // Slice the hidden at the slot that produced the newly-accepted
        // bonus's prediction. Round 1: last (and only) position. Round 2+:
        // index `lastRoundAccepted`, matching mlx-lm's
        // `verify.hidden[:, accepted : accepted + 1, :]` semantic.
        let bonusSlotHidden: MLXArray
        if let idx = lastRoundAccepted {
            bonusSlotHidden = lastHidden[0..., idx ..< (idx + 1), 0...]
        } else {
            bonusSlotHidden = lastHidden[0..., (-1)..., 0...]
        }

        let queryOffset =
            state[mtpSharedKVOffsetsKey]?["full_attention"]
            ?? mainCacheStorage.processedTokenCount

        // Invariant: the snapshot the drafter attends over describes the emitted sequence and
        // nothing else. A global layer's entry spans the whole stream; a sliding layer's spans
        // its window. Both are exact, because the commit that decided what the cache kept also
        // reconciled the snapshot. `dim()` is shape metadata (no eval, no GPU sync).
        assert(
            {
                guard let sources = state[mtpSharedKVSourceIndicesKey] else { return false }
                return sharedKV.allSatisfy { key, entry in
                    guard let leaf = sources[key] else { return false }
                    return entry.0.dim(-2) == mainCacheStorage.emittedLength(forLeaf: leaf)
                }
            }(),
            """
            stale sharedKV: spans \(sharedKV.mapValues { $0.0.dim(-2) }) do not match the \
            emitted lengths implied by a timeline of \(mainCacheStorage.processedTokenCount)
            """
        )

        let bonusToken = y.tokens
        let draftTokens: MLXArray
        let flatDraftTokens: MLXArray
        let pldDraftList: [Int]?
        if let pld = pldProposal {
            let arr = MLXArray(pld.drafts.map(Int32.init))
            flatDraftTokens = arr
            draftTokens = arr.expandedDimensions(axis: 0)
            pldDraftList = pld.drafts
        } else {
            pldDraftList = nil
            if let statefulDrafter = drafter as? any StatefulMTPDrafterModel,
                var currentDrafterState = drafterState
            {
                draftTokens = statefulDrafter.draftBlock(
                    target: mainModel,
                    lastToken: bonusToken,
                    lastHidden: bonusSlotHidden,
                    sharedKV: sharedKV,
                    positionDeltas: state[mtpPositionDeltasKey],
                    queryOffset: queryOffset,
                    blockSize: numDraft + 1,  // total round size: bonus + numDraft
                    state: &currentDrafterState,
                    sampler: draftSampler
                )
                drafterState = currentDrafterState
            } else {
                draftTokens = drafter.draftBlock(
                    target: mainModel,
                    lastToken: bonusToken,
                    lastHidden: bonusSlotHidden,
                    sharedKV: sharedKV,
                    positionDeltas: state[mtpPositionDeltasKey],
                    queryOffset: queryOffset,
                    blockSize: numDraft + 1,  // total round size: bonus + numDraft
                    sampler: draftSampler
                )
            }
            // draftTokens shape [B, numDraft] -> flatten to [numDraft].
            flatDraftTokens = draftTokens.flattened()
        }

        // Verify pass: main model evaluates [bonus, draft_1, ..., draft_numDraft]
        // in one forward call, emitting state for next round.
        var verifyState = state
        verifyState[mtpEmitFlagKey] = true
        let isStatelessProcessor = (processor == nil) || (processor is SuppressTokensProcessor)
        verifyState[mtpSkipLogitSoftcapKey] = (sampler is ArgMaxSampler) && isStatelessProcessor
        let verifyTokens = concatenated([bonusToken, flatDraftTokens])
        let verifyInput = LMInput.Text(tokens: verifyTokens)
        let verifyStart = verifyInput.tokens.dim(0) - (numDraft + 1)
        verifyState[mtpCacheCheckpointIndexKey] = nativeHybridRewind ? 1 : nil
        let verifyCache = nativeHybridRewind ? mainCache : round!.caches
        let mainResult = mainModel(
            verifyInput[text: .newAxis], cache: verifyCache, state: verifyState)
        let mainLogits = mainResult.logits
        mainState = mainResult.state

        var accepted = 0
        var finalToken: MLXArray?

        // Iris: batched greedy verification — compute all `numDraft + 1` argmax tokens on GPU
        // in one op and settle with a single `eval()` instead of `numDraft + 1` serial GPU syncs.
        if sampler is ArgMaxSampler, isStatelessProcessor {
            var verifyLogits = mainLogits[
                0..., verifyStart ..< (verifyStart + numDraft + 1), 0...
            ].reshaped([numDraft + 1, -1])
            if let suppressor = processor as? SuppressTokensProcessor {
                verifyLogits = suppressor.process(logits: verifyLogits)
            }
            let sampledAll = argMax(verifyLogits, axis: -1)  // [numDraft + 1]
            let draftTokensList: [Int]
            if let list = pldDraftList {
                eval(sampledAll)
                draftTokensList = list
            } else {
                eval(flatDraftTokens, sampledAll)
                draftTokensList = flatDraftTokens.asArray(Int.self)
            }
            let sampledList = sampledAll.asArray(Int.self)
            for i in 0 ..< numDraft {
                let targetTokenValue = sampledList[i]
                pendingTokens.append(targetTokenValue)
                recordEmitted(targetTokenValue)
                guard targetTokenValue == draftTokensList[i] else {
                    finalToken = sampledAll[i ..< (i + 1)]
                    break
                }
                accepted += 1
            }
            if finalToken == nil {
                let bonusVal = sampledList[accepted]
                pendingTokens.append(bonusVal)
                recordEmitted(bonusVal)
                finalToken = sampledAll[accepted ..< (accepted + 1)]
            }
        } else {
            let draftTokensList: [Int]
            if let list = pldDraftList {
                draftTokensList = list
            } else {
                eval(flatDraftTokens)
                draftTokensList = flatDraftTokens.asArray(Int.self)
            }
            for i in 0 ..< numDraft {
                var logits = mainLogits[0..., verifyStart + i, 0...]
                logits = processor?.process(logits: logits) ?? logits
                let targetToken = sampler.sample(logits: logits)
                eval(targetToken)
                let targetTokenValue = targetToken.item(Int.self)
                processor?.didSample(token: targetToken)
                pendingTokens.append(targetTokenValue)
                recordEmitted(targetTokenValue)
                guard targetTokenValue == draftTokensList[i] else {
                    finalToken = targetToken
                    break
                }
                accepted += 1
            }

            // Only the all-accepted path samples the bonus row. On rejection the
            // mismatching target sample above is already the emitted correction.
            if finalToken == nil {
                var logits = mainLogits[0..., verifyStart + accepted, 0...]
                logits = processor?.process(logits: logits) ?? logits
                let bonus = sampler.sample(logits: logits)
                eval(bonus)
                processor?.didSample(token: bonus)
                let bonusVal = bonus.item(Int.self)
                pendingTokens.append(bonusVal)
                recordEmitted(bonusVal)
                finalToken = bonus
            }
        }

        if let pld = pldProposal {
            lookupLastAccepted = accepted
            if accepted > 0 {
                lookupCursor = pld.matchEnd + accepted + 1
                lookupMissStreak = 0
            } else {
                lookupMissStreak += 1
            }
            // If PLD accepted fewer than 6 tokens on a non-final span, we hit a typo-dense region:
            // let fast MTP breeze through the next 2 rounds before probing PLD again.
            if accepted < 6 && accepted < numDraft {
                lookupCooldown = 2
            }
        } else {
            lookupCursor = Swift.min(promptLookupTokens.count, lookupCursor + accepted + 1)
            if lookupMissStreak > 0 {
                lookupMissStreak = Swift.max(0, lookupMissStreak - 1)
            }
        }
        let emittedFinalToken = finalToken!
        committedPendingTokenCount = accepted

        proposedCount += numDraft
        acceptedCount += accepted
        lastRoundAccepted = accepted
        if let verifyHidden = mainResult.state?[mtpLastHiddenStatesKey] {
            lastVerifyHidden = verifyHidden
            lastVerifyStart = verifyStart
            lastVerifyAccepted = accepted
            targetBoundaryHidden =
                verifyHidden[0..., (verifyStart + accepted) ..< (verifyStart + accepted + 1), 0...]
        }
        telemetry.recordRound(
            drafted: numDraft,
            accepted: accepted,
            targetVerified: numDraft + 1,
            draftModelCalls: pldProposal == nil ? 1 : 0
        )

        if let statefulDrafter = drafter as? any StatefulMTPDrafterModel,
            var currentDrafterState = drafterState
        {
            statefulDrafter.commitDrafterState(
                target: mainModel,
                targetHidden: mainResult.state?[mtpLastHiddenStatesKey] ?? lastHidden,
                draftTokens: draftTokens,
                acceptedCount: accepted,
                finalToken: emittedFinalToken,
                positionDeltas: mainResult.state?[mtpPositionDeltasKey],
                state: &currentDrafterState,
                sampler: sampler)
            drafterState = currentDrafterState
        }

        let rejected = numDraft - accepted
        let snapshotPlaced: Bool
        if nativeHybridRewind {
            if rejected == 0 {
                mainCacheStorage.commitProcessedTokens(verifyInput.cacheSequenceLength)
            } else {
                let rewound = rewindSpeculativePromptCache(
                    mainCache, numTokens: rejected)
                precondition(
                    rewound == rejected,
                    "Target advertised native speculative rewind depth \(nativeRewindDepth), but rewound \(rewound) of \(rejected) positions"
                )
                // Qwen MTP-1: attention KV trims one token while every GDN
                // cache restores the state captured after the committed bonus.
                // The target's 9B weights are not replayed on rejection.
                mainCacheStorage.commitProcessedTokens(accepted + 1)
            }
            snapshotPlaced = reconcileSharedKVState(
                &mainState, discarding: rejected,
                lengths: mainCacheStorage.emittedLength(forLeaf:))
        } else {
            // Staged layers never held rejected rows; write-through layers clamp back over them.
            let commit = mainCacheStorage.commit(round!, retaining: accepted + 1)
            snapshotPlaced = reconcileSharedKVState(
                &mainState, discarding: rejected,
                lengths: { commit.emittedLengths[$0] })
        }

        guard snapshotPlaced else {
            switchToPassthrough(reason: Self.missingSharedKVSourcesReason)
            mainState = nil
            y = .init(tokens: emittedFinalToken)
            return
        }

        // Dynamic cache quantization may convert regular K/V to quantized K/V,
        // at which point the target's emit-hook cannot provide full_attention
        // shared K/V and the next round transitions to passthrough.
        kvCachePlan.apply(to: mainCacheStorage)

        y = .init(tokens: emittedFinalToken)
    }

    /// Records an emitted token for PLD matching (no-op when PLD is off; keeps only the 4-token
    /// sliding window that `proposeFromPrompt` inspects).
    private mutating func recordEmitted(_ token: Int) {
        guard !promptLookupTokens.isEmpty else { return }
        emittedHistoryCount += 1
        if emittedHistory.count == 4 {
            emittedHistory.removeFirst()
        }
        emittedHistory.append(token)
    }

    /// Iris: Prompt-Lookup Drafting (PLD) matcher against the user's input tokens.
    /// Searches locally near `lookupCursor` first so common phrases advance monotonically,
    /// then falls back to scanning the full draft. Returns nil when no confident match exists
    /// so the MTP drafter seamlessly takes over.
    private func proposeFromPrompt(maxCount: Int) -> (drafts: [Int], matchEnd: Int)? {
        let src = promptLookupTokens
        let srcCount = src.count
        let histCount = emittedHistory.count
        guard srcCount >= 2, emittedHistoryCount >= 1, histCount >= 1, maxCount >= 1 else { return nil }

        // First round right after prefill: if token 0 matches at the start of the draft,
        // propose the following span directly without running the drafter.
        if emittedHistoryCount == 1 {
            let t0 = emittedHistory[0]
            let searchEnd = srcCount < 25 ? 1 : Swift.min(6, srcCount - 1)
            for i in 0 ..< searchEnd where src[i] == t0 {
                let start = i + 1
                let count = Swift.min(maxCount, srcCount - start)
                if count >= 2 {
                    return (Array(src[start ..< (start + count)]), start)
                }
            }
            return nil
        }

        // Helper: find an n-gram match of the last `n` emitted tokens in `src[lo..<hi]`.
        func findMatch(n: Int, from lo: Int, to hi: Int) -> Int? {
            guard lo < hi else { return nil }
            let lastTok = emittedHistory[histCount - 1]
            let baseHist = histCount - n
            var pos = lo
            while pos < hi {
                if src[pos - 1] == lastTok {
                    var ok = true
                    var k = 0
                    while k < n - 1 {
                        if src[pos - n + k] != emittedHistory[baseHist + k] {
                            ok = false
                            break
                        }
                        k += 1
                    }
                    if ok { return pos }
                }
                pos += 1
            }
            return nil
        }

        // Require a 3-gram match by default so typo clusters don't trigger costly PLD passes;
        // allow 2-gram immediately after a high-acceptance (>= 8) PLD span to jump right over
        // a single-word fix back into verbatim copy.
        let minN = (lookupMissStreak == 0 && lookupLastAccepted >= 8) ? 2 : 3
        let maxN = Swift.min(4, histCount)
        let minAvail = srcCount < 25 ? 12 : 6
        if maxN >= minN {
            for n in stride(from: maxN, through: minN, by: -1) {
                let localLo = Swift.max(n, lookupCursor - 8)
                let localHi = Swift.min(srcCount, lookupCursor + 64)
                let matchPos =
                    findMatch(n: n, from: localLo, to: localHi)
                    ?? findMatch(n: n, from: n, to: srcCount)
                if let pos = matchPos {
                    let avail = Swift.min(maxCount, srcCount - pos)
                    if avail >= minAvail || (srcCount >= 25 && avail >= 2 && pos + avail == srcCount) {
                        return (Array(src[pos ..< (pos + avail)]), pos)
                    }
                }
            }
        }

        // 1-gram continuation ONLY at the exact expected cursor right after a high-acceptance
        // PLD span (>= 8 tokens) so a 1-token typo fix immediately resumes full-speed PLD.
        if lookupMissStreak == 0, lookupLastAccepted >= 8, lookupCursor >= 1, lookupCursor < srcCount {
            let lastTok = emittedHistory[histCount - 1]
            let searchLo = Swift.max(1, lookupCursor - 2)
            let searchHi = Swift.min(srcCount, lookupCursor + 4)
            for pos in searchLo ..< searchHi where src[pos - 1] == lastTok {
                let avail = Swift.min(maxCount, srcCount - pos)
                if avail >= 6 || (avail >= 2 && pos + avail == srcCount) {
                    return (Array(src[pos ..< (pos + avail)]), pos)
                }
            }
        }

        return nil
    }

    /// Switch to single-token generation for the remainder of the stream.
    /// Sticky — once flipped, `next()` never returns to speculation.
    private mutating func switchToPassthrough(reason: String) {
        if !passthroughLoggedOnce {
            // Iris: log to stderr rather than stdout so processes that use stdout for IPC
            // (like Iris's brain) never receive diagnostic text in their protocol stream.
            if let data = "[MTPSpeculativeTokenIterator] passthrough mode: \(reason)\n".data(using: .utf8) {
                FileHandle.standardError.write(data)
            }
            passthroughLoggedOnce = true
        }
        passthroughReason = reason
        passthrough = true
        discardSpeculativePromptCacheCheckpoints(mainCache)
        mainState?[mtpEmitFlagKey] = false
        mainState?[mtpSkipLogitSoftcapKey] = nil
        mainState?[mtpCacheCheckpointIndexKey] = nil
        mainState?[mtpLastHiddenStatesKey] = nil
        mainState?[mtpSharedKVStatesKey] = nil
        mainState?[mtpSharedKVSourceIndicesKey] = nil
        mainState?[mtpSharedKVOffsetsKey] = nil
        mainState?[mtpPositionDeltasKey] = nil
    }

    /// One single-token forward step against the main model.
    ///
    /// Sticky passthrough leaves the drafter alone. A one-token generation
    /// tail can instead advance stateful drafter state as an empty proposal,
    /// preserving an exact continuation at the final target boundary.
    private mutating func passthroughStep(
        maintainingDrafterState: Bool = false
    ) -> Int? {
        if let maxTokens, tokenCount >= maxTokens { return nil }

        var stepState = mainState
        if maintainingDrafterState {
            stepState = stepState ?? LMOutput.State()
            stepState?[mtpEmitFlagKey] = true
        }
        let result = mainModel(y[text: .newAxis], cache: mainCache, state: stepState)
        mainState = result.state
        mainCacheStorage.commitProcessedTokens(y.cacheSequenceLength)
        var logits = result.logits[0..., -1, 0...]
        logits = processor?.process(logits: logits) ?? logits
        let token = sampler.sample(logits: logits)
        processor?.didSample(token: token)
        eval(token)
        let tokenInt = token.item(Int.self)

        if maintainingDrafterState,
            let statefulDrafter = drafter as? any StatefulMTPDrafterModel,
            var currentDrafterState = drafterState,
            let targetHidden = result.state?[mtpLastHiddenStatesKey],
            targetHidden.dim(1) > 0
        {
            statefulDrafter.commitDrafterState(
                target: mainModel,
                targetHidden: targetHidden,
                draftTokens: MLXArray.zeros([0], dtype: token.dtype),
                acceptedCount: 0,
                finalToken: token,
                positionDeltas: result.state?[mtpPositionDeltasKey],
                state: &currentDrafterState,
                sampler: sampler)
            drafterState = currentDrafterState
            targetBoundaryHidden = targetHidden[0..., (-1)..., 0...]
        } else if maintainingDrafterState,
            drafter is any StatefulMTPDrafterModel
        {
            switchToPassthrough(
                reason:
                    "MTP drafter could not follow the final target-only token; continuation discarded"
            )
        }

        y = .init(tokens: token)
        kvCachePlan.apply(to: mainCacheStorage)
        return tokenInt
    }

    public mutating func next() -> Int? {
        if let maxTokens, tokenCount >= maxTokens {
            return nil
        }

        // Drain the pending buffer first — before the passthrough branch. A
        // stand-down at the end of `init` leaves the prepare-time bonus
        // sitting here, already committed to the cache (`y` is the token
        // after it). Short-circuiting to `passthroughStep()` would drop it and
        // start the stream one position ahead of an equivalent autoregressive
        // run. Rounds that flip passthrough mid-stream always clear the buffer
        // first, so this reordering is a no-op for them.
        if pendingIndex < pendingTokens.count {
            let token = pendingTokens[pendingIndex]
            pendingIndex += 1
            telemetry.recordGeneratedToken()
            return token
        }

        if passthrough {
            if let token = passthroughStep() {
                telemetry.recordGeneratedToken()
                return token
            }
            return nil
        }

        // Run a new speculation round (may transition to passthrough).
        pendingTokens.removeAll(keepingCapacity: true)
        pendingIndex = 0
        committedPendingTokenCount = 0
        autoreleasepool { speculateRound() }

        if pendingTokens.isEmpty {
            // speculateRound chose passthrough -- fall through.
            if passthrough {
                if let token = passthroughStep() {
                    telemetry.recordGeneratedToken()
                    return token
                }
            }
            return nil
        }

        let token = pendingTokens[pendingIndex]
        pendingIndex += 1
        telemetry.recordGeneratedToken()
        return token
    }

}

extension MTPSpeculativeTokenIterator: GenerationFinalizingTokenIterator {
    mutating func finalizeGeneration() {
        defer { processor?.finalizeGeneration() }
        // A fully consumed all-accepted round can still retain the recurrent
        // checkpoint used for early-finalization rollback. Release it even
        // when no committed lookahead remains.
        var discardedTargetTokens = 0
        defer {
            discardSpeculativePromptCacheCheckpoints(mainCache)
            finalizeDrafterContinuation(discardedTargetTokens: discardedTargetTokens)
        }
        let consumed = Swift.min(pendingIndex, committedPendingTokenCount)
        let lookahead = committedPendingTokenCount - consumed
        guard lookahead > 0 else { return }

        let usesNativeHybridRewind =
            ((mainModel as? any SpeculativeCacheRewindModel)?
                .maximumNativeTargetCacheRewind ?? 0) >= blockSize - 1
            && mainCache.contains { $0 is MambaCache }
            && mainCache.allSatisfy { $0.isTrimmable || $0 is MambaCache }
        let rewound: Int
        if usesNativeHybridRewind {
            let trimmed = mainCacheStorage.trim(lookahead)
            rewound = trimmed > 0 ? trimmed : mainCacheStorage.rewindSpeculative(lookahead)
        } else {
            // A staged round keeps what a wrapped ring needs to undo committed lookahead exactly.
            rewound = mainCacheStorage.rewindLastRound(lookahead)
        }
        discardedTargetTokens = rewound
        if rewound == lookahead, let lastVerifyHidden {
            let retainedAccepted = lastVerifyAccepted - lookahead
            targetBoundaryHidden =
                lastVerifyHidden[
                    0...,
                    (lastVerifyStart + retainedAccepted)
                        ..< (lastVerifyStart + retainedAccepted + 1),
                    0...
                ]
        } else if rewound != lookahead {
            // The target cache itself remains authoritative, but there is no
            // exact hidden-state witness from which a drafter may resume.
            targetBoundaryHidden = nil
        }
        // The only site that ignores the result, and the only one that can: the stream is over,
        // so nothing reads the snapshot after this. Everywhere else a refusal stops speculation.
        reconcileSharedKVState(
            &mainState, discarding: rewound,
            lengths: mainCacheStorage.emittedLength(forLeaf:))
    }

    private mutating func finalizeDrafterContinuation(discardedTargetTokens: Int) {
        finalizedContinuation = nil
        guard !passthrough else { return }
        guard let resumableDrafter = drafter as? any ResumableMTPDrafterModel else { return }
        guard mainCacheStorage.processedTokenCount > 0 else { return }
        guard var currentDrafterState = drafterState, let targetBoundaryHidden else {
            passthroughReason =
                "MTP continuation boundary could not be normalized; continuation discarded"
            return
        }

        guard
            resumableDrafter.finalizeDrafterState(
                target: mainModel,
                targetBoundaryHidden: targetBoundaryHidden,
                targetProcessedTokenCount: mainCacheStorage.processedTokenCount,
                discardedTargetTokens: discardedTargetTokens,
                positionDeltas: mainState?[mtpPositionDeltasKey],
                state: &currentDrafterState)
        else {
            passthroughReason =
                "MTP drafter state could not be normalized; continuation discarded"
            return
        }

        // The continuation crosses back from the generation task only after
        // all of its lazy MLX values have been settled.
        var arrays = [targetBoundaryHidden]
        arrays.append(contentsOf: currentDrafterState.cache.flatMap(\.state))
        if let seedToken = currentDrafterState.seedToken { arrays.append(seedToken) }
        if let seedHidden = currentDrafterState.seedHidden { arrays.append(seedHidden) }
        eval(arrays)

        drafterState = currentDrafterState
        finalizedContinuation = MTPDrafterContinuation(
            state: currentDrafterState,
            targetBoundaryHidden: targetBoundaryHidden,
            targetProcessedTokenCount: mainCacheStorage.processedTokenCount)
    }
}

package protocol MTPDrafterContinuationProviding {
    var mtpDrafterContinuation: MTPDrafterContinuation? { get }
}

extension MTPSpeculativeTokenIterator: MTPDrafterContinuationProviding {
    package var mtpDrafterContinuation: MTPDrafterContinuation? {
        finalizedContinuation
    }
}

extension MTPSpeculativeTokenIterator: MTPStatsCollecting {
    public var proposedDraftTokens: Int { proposedCount }
    public var acceptedDraftTokens: Int { acceptedCount }
}

extension MTPSpeculativeTokenIterator {
    /// Test-only setter for the canonical `LogitProcessor`. Lets regression
    /// tests install a recording probe AFTER `init` (which calls `prepare`
    /// and would otherwise consume the prepare-time bonus before the probe
    /// is observable). Used by the emit-only invariant regression tests in
    /// `MTPSpeculativeTokenIteratorTests` (CI-scoped) and
    /// `MTPIteratorEndToEndDiagnosticTests` (31B end-to-end).
    @_spi(Testing) public mutating func _setProcessorForTesting(
        _ processor: LogitProcessor?
    ) {
        self.processor = processor
    }

    /// Test-only getter for the canonical `LogitProcessor` so regression
    /// tests can inspect its post-drain state (e.g., a recording probe's
    /// accumulated didSample log).
    @_spi(Testing) public var _processorForTesting: LogitProcessor? {
        processor
    }
}

/// Remove MTP's transient target↔drafter exchange values while retaining
/// model-owned continuation state such as Qwen VLM M-RoPE anchors.
private func persistentTargetState(from state: LMOutput.State?) -> LMOutput.State? {
    guard var state else { return nil }
    state[mtpEmitFlagKey] = nil
    state[mtpSkipLogitSoftcapKey] = nil
    state[mtpCacheCheckpointIndexKey] = nil
    state[mtpLastHiddenStatesKey] = nil
    state[mtpSharedKVStatesKey] = nil
    state[mtpSharedKVSourceIndicesKey] = nil
    state[mtpSharedKVOffsetsKey] = nil
    state[mtpPositionDeltasKey] = nil
    return state.isEmpty ? nil : state
}

/// Bring the emitted MTP shared-K/V snapshot into line with what the cache actually kept.
///
/// The verify pass emits K/V spanning its whole chunk, materialized before acceptance is known,
/// and a sliding layer is presented more than its window on purpose so every query in the chunk
/// still sees a full one. The drafter has no cache of its own and attends over this snapshot
/// bidirectionally, so both excesses have to go: the rejected tail, then the head beyond what the
/// layer can still describe.
///
/// Order matters. Dropping the tail first and clamping the head second leaves a sliding entry
/// with a full window; clamping first would leave it `discarding` rows short, every round.
///
/// - Parameters:
///   - discarding: trailing positions the commit did not keep.
///   - lengths: for the cache entry behind a given key, how much of the emitted sequence it still
///     describes — the whole stream for a global layer, the trailing window for a sliding one.
///
/// No-op when `state` is nil or carries no snapshot (the quantization-onset round emits none).
/// Returns `false` when the snapshot cannot be placed against cache entries at all, which is a
/// signal to stop speculating rather than reconcile against a guess. Cost is metadata-only: every
/// slice is a lazy view consumed by the next `draftBlock` like the rest of the round's inputs.
@discardableResult
func reconcileSharedKVState(
    _ state: inout LMOutput.State?, discarding: Int, lengths: (Int) -> Int
) -> Bool {
    guard let sharedKV = state?[mtpSharedKVStatesKey] else { return true }
    guard let sources = state?[mtpSharedKVSourceIndicesKey],
        sharedKV.keys.allSatisfy({ sources[$0] != nil })
    else { return false }

    state?[mtpSharedKVStatesKey] = sharedKV.reduce(into: [:]) { result, entry in
        let (key, kv) = entry
        var length = kv.0.dim(-2)
        if discarding > 0 {
            length = Swift.max(0, length - discarding)
        }
        let bound = lengths(sources[key]!)
        let start = Swift.max(0, length - bound)
        result[key] = (
            kv.0[.ellipsis, start ..< length, 0...],
            kv.1[.ellipsis, start ..< length, 0...]
        )
    }
    if discarding > 0, let offsets = state?[mtpSharedKVOffsetsKey] {
        state?[mtpSharedKVOffsetsKey] = offsets.mapValues {
            Swift.max(0, $0 - discarding)
        }
    }
    return true
}
