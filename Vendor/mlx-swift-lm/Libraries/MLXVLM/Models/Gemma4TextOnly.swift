// Iris: Gemma 4 for text only, with less memory (see IRIS-CHANGES.md at the package root).

import Foundation
import MLX
import MLXLMCommon

/// Set before loading a Gemma 4 model to build it for text only, with less memory:
/// - the per-layer word table (`embed_tokens_per_layer`, 1.3 to 1.6 GB) stays in the model's
///   file and the rows each token needs are read from it, instead of the table being held in
///   memory. The same 4-bit rows, dequantized the same way, so the answers are the same;
/// - the picture encoder is built with no layers and its weights are left out.
///
/// Read once, when the model is built: changing it later doesn't affect a loaded model. Nil
/// (the default) builds the stock model.
public enum Gemma4TextOnly {
    @TaskLocal private static var scopedWordTableFile: URL?
    private static let lock = NSLock()
    nonisolated(unsafe) private static var _wordTableFile: URL?

    /// The model's own `model.safetensors`, whose word table is read row by row.
    /// Prefers the task-local binding from ``withWordTableFile(_:operation:)`` so overlapping
    /// loads cannot clobber each other.
    public static var wordTableFile: URL? {
        get { scopedWordTableFile ?? lock.withLock { _wordTableFile } }
        set { lock.withLock { _wordTableFile = newValue } }
    }

    /// Runs `operation` with `file` bound to this task tree only.
    public static func withWordTableFile<T>(
        _ file: URL?, operation: () async throws -> T
    ) async rethrows -> T {
        try await $scopedWordTableFile.withValue(file, operation: operation)
    }

    /// The vision configuration with no encoder layers, for a text-only build.
    static func withoutLayers(_ config: Gemma4VisionConfiguration) -> Gemma4VisionConfiguration {
        guard var json = (try? JSONSerialization.jsonObject(with: JSONEncoder().encode(config))) as? [String: Any] else {
            return config
        }
        json["num_hidden_layers"] = 0
        guard let data = try? JSONSerialization.data(withJSONObject: json),
            let trimmed = try? JSONDecoder().decode(Gemma4VisionConfiguration.self, from: data)
        else { return config }
        return trimmed
    }
}

/// The per-layer word table, read from the model's safetensors file a row at a time, with a
/// bounded in-memory cache of the quantized rows recently seen (~12 MB cap for 2,048 tokens)
/// so repeated tokens avoid `pread` syscalls while keeping the 1.3–1.6 GB table off RAM.
final class Gemma4WordTableOnDisk: @unchecked Sendable {
    private struct Part {
        let offset: Int
        let rowBytes: Int
        let columns: Int
        let dtype: DType
    }

    private let fd: Int32
    private let weight: Part, scales: Part, biases: Part
    private let groupSize: Int, bits: Int
    private let lock = NSLock()
    private let entryBytes: Int
    private let cacheCapacity = 2048
    /// Token ID -> slot index in `cacheBuf`.
    private var cacheIndex: [Int32: Int] = [:]
    private var cacheSlots: [Int32] = []
    private var cacheBuf: [UInt8] = []
    private var nextSlot = 0

    /// Nil when the file doesn't hold the table as expected (then the stock model is built).
    /// `rowWidth`: the table's unquantized row width, layers × per-layer input size.
    /// `rowCount`: minimum rows the model's per-layer vocabulary requires.
    init?(file: URL, rowWidth: Int, rowCount: Int = 0) {
        guard rowWidth > 0, rowCount >= 0 else { return nil }
        let f = open(file.path, O_RDONLY)
        guard f >= 0 else { return nil }
        var st = stat()
        guard fstat(f, &st) == 0, st.st_size > 8 else { close(f); return nil }
        let fileSize = Int64(st.st_size)
        var headerLength: UInt64 = 0
        guard pread(f, &headerLength, 8, 0) == 8,
            headerLength > 0, headerLength < 100_000_000,
            Int64(8) + Int64(headerLength) <= fileSize
        else { close(f); return nil }
        var header = Data(count: Int(headerLength))
        let read = header.withUnsafeMutableBytes { buf -> ssize_t in
            guard let base = buf.baseAddress else { return -1 }
            return pread(f, base, Int(headerLength), 8)
        }
        guard read == Int(headerLength),
            let entries = try? JSONSerialization.jsonObject(with: header) as? [String: Any]
        else { close(f); return nil }
        let dataStart = Int64(8) + Int64(headerLength)
        func part(_ name: String) -> Part? {
            guard let entry = entries["language_model.model.embed_tokens_per_layer." + name] as? [String: Any],
                let shape = entry["shape"] as? [Int], shape.count == 2,
                shape[0] > 0, shape[0] >= rowCount, shape[1] > 0,
                let offsets = entry["data_offsets"] as? [Int], offsets.count == 2,
                offsets[0] >= 0, offsets[1] >= offsets[0],
                let type = entry["dtype"] as? String
            else { return nil }
            let dtype: DType
            switch type {
            case "U32": dtype = .uint32
            case "BF16": dtype = .bfloat16
            case "F16": dtype = .float16
            case "F32": dtype = .float32
            default: return nil
            }
            let (rowBytes, rbOverflow) = shape[1].multipliedReportingOverflow(by: dtype.size)
            guard !rbOverflow, rowBytes > 0 else { return nil }
            let (tensorBytes, tbOverflow) = Int64(shape[0]).multipliedReportingOverflow(by: Int64(rowBytes))
            guard !tbOverflow else { return nil }
            let (partOffset, offOverflow) = dataStart.addingReportingOverflow(Int64(offsets[0]))
            guard !offOverflow, partOffset <= Int64(Int.max) else { return nil }
            let (partEnd, endOverflow) = partOffset.addingReportingOverflow(tensorBytes)
            guard !endOverflow, partEnd <= fileSize, Int64(offsets[1] - offsets[0]) >= tensorBytes else {
                return nil
            }
            return Part(offset: Int(partOffset), rowBytes: rowBytes, columns: shape[1], dtype: dtype)
        }
        guard let weight = part("weight"), let scales = part("scales"), let biases = part("biases"),
            weight.dtype == .uint32, scales.columns > 0, biases.columns == scales.columns,
            rowWidth % scales.columns == 0,
            (weight.columns * 32) % rowWidth == 0
        else { close(f); return nil }
        let bits = weight.columns * 32 / rowWidth
        guard [2, 3, 4, 5, 6, 8].contains(bits) else { close(f); return nil }
        let (wsBytes, o1) = weight.rowBytes.addingReportingOverflow(scales.rowBytes)
        let (totalEntryBytes, o2) = wsBytes.addingReportingOverflow(biases.rowBytes)
        guard !o1, !o2, totalEntryBytes > 0 else { close(f); return nil }
        self.fd = f
        self.weight = weight
        self.scales = scales
        self.biases = biases
        self.groupSize = rowWidth / scales.columns
        self.bits = bits
        self.entryBytes = totalEntryBytes
    }

    deinit { close(fd) }

    /// Reads one token's `(weight, scales, biases)` quantized bytes into `cacheBuf` if not cached,
    /// returning its byte offset inside `cacheBuf`. Caller holds `lock`.
    private func cachedOffset(for id: Int32) -> Int {
        if let slot = cacheIndex[id] {
            return slot * entryBytes
        }
        let slot: Int
        if cacheSlots.count < cacheCapacity {
            slot = cacheSlots.count
            cacheSlots.append(id)
            cacheBuf.append(contentsOf: repeatElement(UInt8(0), count: entryBytes))
        } else {
            slot = nextSlot
            nextSlot = (nextSlot + 1) % cacheCapacity
            cacheIndex.removeValue(forKey: cacheSlots[slot])
            cacheSlots[slot] = id
        }
        let base = slot * entryBytes
        let wOff = base
        let sOff = wOff + weight.rowBytes
        let bOff = sOff + scales.rowBytes
        cacheBuf.withUnsafeMutableBytes { buf in
            let ptr = buf.baseAddress!
            let gw = pread(fd, ptr + wOff, weight.rowBytes, off_t(weight.offset + Int(id) * weight.rowBytes))
            let gs = pread(fd, ptr + sOff, scales.rowBytes, off_t(scales.offset + Int(id) * scales.rowBytes))
            let gb = pread(fd, ptr + bOff, biases.rowBytes, off_t(biases.offset + Int(id) * biases.rowBytes))
            precondition(
                gw == weight.rowBytes && gs == scales.rowBytes && gb == biases.rowBytes,
                "the model's word table couldn't be read")
        }
        cacheIndex[id] = slot
        return base
    }

    /// The table's rows for `tokens`, dequantized: shape `tokens.shape + [rowWidth]`.
    func lookup(_ tokens: MLXArray) -> MLXArray {
        let shape = tokens.shape
        let flat = tokens.flattened().asType(.int32)
        eval(flat)
        let ids = flat.asArray(Int32.self)
        let n = ids.count
        if n == 0 {
            let empty = MLXArray.zeros([0, weight.columns * 32 / bits], dtype: scales.dtype)
            return empty.reshaped(shape + [-1])
        }
        var wData = Data(count: n * weight.rowBytes)
        var sData = Data(count: n * scales.rowBytes)
        var bData = Data(count: n * biases.rowBytes)
        lock.lock()
        let wBytes = weight.rowBytes
        let sBytes = scales.rowBytes
        let bBytes = biases.rowBytes
        // Copy each row out immediately after resolving its slot so a later miss in the same
        // call cannot evict and overwrite an earlier hit's slot before memcpy reads it.
        wData.withUnsafeMutableBytes { wDst in
            sData.withUnsafeMutableBytes { sDst in
                bData.withUnsafeMutableBytes { bDst in
                    let wp = wDst.baseAddress!
                    let sp = sDst.baseAddress!
                    let bp = bDst.baseAddress!
                    for i in 0 ..< n {
                        let off = cachedOffset(for: ids[i])
                        cacheBuf.withUnsafeBytes { src in
                            let srcBase = src.baseAddress!
                            memcpy(wp + i * wBytes, srcBase + off, wBytes)
                            memcpy(sp + i * sBytes, srcBase + off + wBytes, sBytes)
                            memcpy(bp + i * bBytes, srcBase + off + wBytes + sBytes, bBytes)
                        }
                    }
                }
            }
        }
        lock.unlock()
        let wArr = MLXArray(wData, [n, weight.columns], dtype: weight.dtype)
        let sArr = MLXArray(sData, [n, scales.columns], dtype: scales.dtype)
        let bArr = MLXArray(bData, [n, biases.columns], dtype: biases.dtype)
        let rows = dequantized(wArr, scales: sArr, biases: bArr, groupSize: groupSize, bits: bits)
        return rows.reshaped(shape + [-1])
    }
}
