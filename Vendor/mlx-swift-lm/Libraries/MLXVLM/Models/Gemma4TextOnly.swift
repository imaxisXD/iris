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
    private static let lock = NSLock()
    nonisolated(unsafe) private static var _wordTableFile: URL?

    /// The model's own `model.safetensors`, whose word table is read row by row.
    public static var wordTableFile: URL? {
        get { lock.withLock { _wordTableFile } }
        set { lock.withLock { _wordTableFile = newValue } }
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

/// The per-layer word table, read from the model's safetensors file a row at a time.
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

    /// Nil when the file doesn't hold the table as expected (then the stock model is built).
    /// `rowWidth`: the table's unquantized row width, layers × per-layer input size.
    init?(file: URL, rowWidth: Int) {
        let f = open(file.path, O_RDONLY)
        guard f >= 0 else { return nil }
        var headerLength: UInt64 = 0
        guard pread(f, &headerLength, 8, 0) == 8, headerLength < 100_000_000 else { close(f); return nil }
        var header = Data(count: Int(headerLength))
        let read = header.withUnsafeMutableBytes { pread(f, $0.baseAddress!, Int(headerLength), 8) }
        guard read == Int(headerLength),
            let entries = try? JSONSerialization.jsonObject(with: header) as? [String: Any]
        else { close(f); return nil }
        func part(_ name: String) -> Part? {
            guard let entry = entries["language_model.model.embed_tokens_per_layer." + name] as? [String: Any],
                let shape = entry["shape"] as? [Int], shape.count == 2,
                let offsets = entry["data_offsets"] as? [Int], offsets.count == 2,
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
            return Part(offset: 8 + Int(headerLength) + offsets[0], rowBytes: shape[1] * dtype.size,
                        columns: shape[1], dtype: dtype)
        }
        guard let weight = part("weight"), let scales = part("scales"), let biases = part("biases"),
            weight.dtype == .uint32, scales.columns > 0, rowWidth % scales.columns == 0,
            (weight.columns * 32) % rowWidth == 0
        else { close(f); return nil }
        let bits = weight.columns * 32 / rowWidth
        guard [2, 3, 4, 5, 6, 8].contains(bits) else { close(f); return nil }
        self.fd = f
        self.weight = weight
        self.scales = scales
        self.biases = biases
        self.groupSize = rowWidth / scales.columns
        self.bits = bits
    }

    deinit { close(fd) }

    private func rows(_ part: Part, _ ids: [Int32]) -> MLXArray {
        var data = Data(count: ids.count * part.rowBytes)
        data.withUnsafeMutableBytes { buffer in
            for (i, id) in ids.enumerated() {
                let got = pread(fd, buffer.baseAddress! + i * part.rowBytes, part.rowBytes,
                                off_t(part.offset + Int(id) * part.rowBytes))
                precondition(got == part.rowBytes, "the model's word table couldn't be read")
            }
        }
        return MLXArray(data, [ids.count, part.columns], dtype: part.dtype)
    }

    /// The table's rows for `tokens`, dequantized: shape `tokens.shape + [rowWidth]`.
    func lookup(_ tokens: MLXArray) -> MLXArray {
        let shape = tokens.shape
        let flat = tokens.flattened().asType(.int32)
        eval(flat)
        let ids = flat.asArray(Int32.self)
        lock.lock()
        defer { lock.unlock() }
        let rows = dequantized(rows(weight, ids), scales: rows(scales, ids), biases: rows(biases, ids),
                               groupSize: groupSize, bits: bits)
        return rows.reshaped(shape + [-1])
    }
}
