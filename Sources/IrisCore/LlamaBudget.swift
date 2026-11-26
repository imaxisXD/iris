import Foundation

/// Memory and slot sizing for `llama-server` (Windows / llama.cpp engine), and the
/// automatic threshold for Iris's "Go easy on this Mac / PC" setting.
///
/// Kept pure in `IrisCore` so both macOS and Windows CI test the exact tiers without
/// launching a server.
public struct LlamaBudget: Equatable, Sendable {
    /// Concurrent prompt slots (`-np`). Fewer slots cut private KV memory linearly;
    /// saved tone prompts in `slots/*.bin` still restore from disk in milliseconds.
    public let slots: Int
    /// Context tokens reserved per slot (`-c = slots * contextPerSlot`).
    public let contextPerSlot: Int
    /// Upper bound on llama-server's in-RAM prompt cache (`--cache-ram`, MB).
    public let cacheRamMB: Int
    /// Skip repacking weights in RAM (`--no-repack`), saving ~0.9 GB private RAM.
    public let noRepack: Bool

    public var totalContext: Int { slots * contextPerSlot }

    public init(slots: Int, contextPerSlot: Int = 6144, cacheRamMB: Int, noRepack: Bool) {
        self.slots = max(1, slots)
        self.contextPerSlot = max(2048, contextPerSlot)
        self.cacheRamMB = max(0, cacheRamMB)
        self.noRepack = noRepack
    }

    /// Machines at or below this RAM size turn on "Go easy on this Mac / PC" by default.
    public static let lightModeMemoryGB = 8

    /// Default value for "Go easy on this Mac / PC" when the user hasn't explicitly chosen.
    public static func recommendedLightMode(memoryGB: Int) -> Bool {
        memoryGB <= lightModeMemoryGB
    }

    /// Server budget for a machine with `memoryGB` of physical RAM and `lightMode` on/off.
    ///
    /// - 4 GB PCs always get the leanest 1-slot / 64 MB budget so Windows stops paging out
    ///   mapped GGUF weights under memory pressure.
    /// - 8 GB PCs get 1 slot / 64 MB in light mode (the default) or 2 slots / 128 MB when
    ///   the user explicitly turns light mode off.
    /// - Roomier machines (> 8 GB) get 2 slots / 128 MB when light mode is turned on, or
    ///   the full 4 slots / 512 MB when off (keeping `--no-repack` below 12 GB).
    public static func forMachine(memoryGB: Int, lightMode: Bool) -> LlamaBudget {
        if memoryGB <= 4 || (lightMode && memoryGB <= 8) {
            return LlamaBudget(slots: 1, contextPerSlot: 6144, cacheRamMB: 64, noRepack: true)
        }
        if memoryGB <= 8 || lightMode {
            return LlamaBudget(slots: 2, contextPerSlot: 6144, cacheRamMB: 128, noRepack: true)
        }
        return LlamaBudget(slots: 4, contextPerSlot: 6144, cacheRamMB: 512, noRepack: memoryGB < 12)
    }
}
