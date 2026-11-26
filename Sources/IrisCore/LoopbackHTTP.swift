// Pure HTTP/1.1 request encoding and streaming response/SSE parsing for loopback llama-server.
// Lives in IrisCore (Foundation only, no OS socket imports) so the exact wire parser that drives
// IrisLlama on Windows and macOS is unit-tested on both platforms.

import Foundation

public enum LoopbackHTTPError: Error, Equatable, Sendable {
    case malformedHeaders
    case malformedChunk(String)
    case httpStatus(Int, String)
    case unexpectedEOF
}

public enum LoopbackHTTPRequest {
    /// Encodes a single HTTP/1.1 request to `127.0.0.1:<port>` with `Connection: close`.
    public static func encode(
        method: String,
        path: String,
        port: Int,
        headers: [(String, String)] = [],
        body: Data? = nil
    ) -> Data {
        var head = "\(method) \(path) HTTP/1.1\r\n"
        head += "Host: 127.0.0.1:\(port)\r\n"
        head += "Connection: close\r\n"
        for (name, value) in headers {
            head += "\(name): \(value)\r\n"
        }
        if let body {
            head += "Content-Length: \(body.count)\r\n"
        }
        head += "\r\n"
        var data = Data(head.utf8)
        if let body, !body.isEmpty {
            data.append(body)
        }
        return data
    }
}

/// Incremental HTTP/1.1 + chunked-transfer + OpenAI SSE (`choices[0].delta.content`) parser.
/// Handles arbitrary TCP read boundaries: split headers, split chunk sizes, split CRLFs,
/// split multi-byte UTF-8 across chunks, and non-200 error bodies.
public struct LoopbackHTTPStream: Sendable {
    private enum Framing: Equatable, Sendable {
        case headers
        case chunkSize
        case chunkData(Int)
        case chunkCRLF
        case contentLength(Int)
        case eof
        case done
    }

    private let parseSSE: Bool
    private var framing: Framing = .headers
    private var raw: [UInt8] = []
    private var rawOffset = 0
    private var body: [UInt8] = []
    private var bodyOffset = 0

    public private(set) var statusCode: Int?
    public private(set) var headersComplete = false
    public private(set) var isComplete = false
    public private(set) var errorBody = ""

    public init(parseSSE: Bool = true) {
        self.parseSSE = parseSSE
    }

    /// Feeds raw bytes read from the socket and returns any newly completed SSE text deltas.
    public mutating func feed(_ chunk: Data) throws -> [String] {
        guard !chunk.isEmpty, !isComplete else { return [] }
        raw.append(contentsOf: chunk)

        parseLoop: while true {
            switch framing {
            case .headers:
                guard let (headerEnd, delimiterLen) = findHeaderEnd() else {
                    if raw.count - rawOffset > 65_536 {
                        throw LoopbackHTTPError.malformedHeaders
                    }
                    break parseLoop
                }
                let headerBytes = raw[rawOffset..<headerEnd]
                rawOffset = headerEnd + delimiterLen

                let lines = headerBytes.split(separator: 0x0A, omittingEmptySubsequences: false).map { lineBytes -> String in
                    let trimmed = lineBytes.last == 0x0D ? lineBytes.dropLast() : lineBytes
                    return String(decoding: trimmed, as: UTF8.self)
                }
                guard let statusLine = lines.first else {
                    throw LoopbackHTTPError.malformedHeaders
                }
                let parts = statusLine.split(separator: " ", maxSplits: 2, omittingEmptySubsequences: true)
                guard parts.count >= 2,
                      parts[0].hasPrefix("HTTP/"),
                      let code = Int(parts[1]) else {
                    throw LoopbackHTTPError.malformedHeaders
                }
                // Skip interim 1xx headers (e.g. 100 Continue) if a server ever sends them.
                if (100..<200).contains(code) { continue parseLoop }

                statusCode = code
                headersComplete = true

                if !parseSSE {
                    framing = .done
                    isComplete = true
                    break parseLoop
                }

                var isChunked = false
                var contentLength: Int?
                for line in lines.dropFirst() {
                    guard let colon = line.firstIndex(of: ":") else { continue }
                    let name = line[..<colon].trimmingCharacters(in: .whitespaces).lowercased()
                    let value = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
                    if name == "transfer-encoding", value.lowercased().contains("chunked") {
                        isChunked = true
                    } else if name == "content-length", let n = Int(value), n >= 0 {
                        contentLength = n
                    }
                }

                if isChunked {
                    framing = .chunkSize
                } else if let len = contentLength {
                    if len == 0 {
                        framing = .done
                        isComplete = true
                        break parseLoop
                    } else {
                        framing = .contentLength(len)
                    }
                } else {
                    framing = .eof
                }

            case .chunkSize:
                guard let nl = findByte(0x0A, in: raw, from: rawOffset) else {
                    if raw.count - rawOffset > 1024 {
                        throw LoopbackHTTPError.malformedChunk("size line too long")
                    }
                    break parseLoop
                }
                let end = (nl > rawOffset && raw[nl - 1] == 0x0D) ? (nl - 1) : nl
                let line = String(decoding: raw[rawOffset..<end], as: UTF8.self)
                rawOffset = nl + 1

                let hexPart = (line.split(separator: ";", maxSplits: 1, omittingEmptySubsequences: false).first.map(String.init) ?? "")
                    .trimmingCharacters(in: .whitespaces)
                guard !hexPart.isEmpty, let size = Int(hexPart, radix: 16), size >= 0 else {
                    throw LoopbackHTTPError.malformedChunk(line)
                }
                if size == 0 {
                    framing = .done
                    isComplete = true
                    break parseLoop
                } else {
                    framing = .chunkData(size)
                }

            case .chunkData(let remaining):
                let available = raw.count - rawOffset
                if available == 0 { break parseLoop }
                let take = min(remaining, available)
                body.append(contentsOf: raw[rawOffset..<(rawOffset + take)])
                rawOffset += take
                if take == remaining {
                    framing = .chunkCRLF
                } else {
                    framing = .chunkData(remaining - take)
                    break parseLoop
                }

            case .chunkCRLF:
                let available = raw.count - rawOffset
                if available == 0 { break parseLoop }
                if raw[rawOffset] == 0x0D {
                    if available < 2 { break parseLoop }
                    guard raw[rawOffset + 1] == 0x0A else {
                        throw LoopbackHTTPError.malformedChunk("expected CRLF")
                    }
                    rawOffset += 2
                } else if raw[rawOffset] == 0x0A {
                    rawOffset += 1
                } else {
                    throw LoopbackHTTPError.malformedChunk("expected CRLF")
                }
                framing = .chunkSize

            case .contentLength(let remaining):
                let available = raw.count - rawOffset
                if available == 0 { break parseLoop }
                let take = min(remaining, available)
                body.append(contentsOf: raw[rawOffset..<(rawOffset + take)])
                rawOffset += take
                if take == remaining {
                    framing = .done
                    isComplete = true
                } else {
                    framing = .contentLength(remaining - take)
                }
                break parseLoop

            case .eof:
                if rawOffset < raw.count {
                    body.append(contentsOf: raw[rawOffset..<raw.count])
                    rawOffset = raw.count
                }
                break parseLoop

            case .done:
                break parseLoop
            }
        }

        compact(&raw, offset: &rawOffset)
        let deltas = drainLines(flushTail: false)
        compact(&body, offset: &bodyOffset)
        return deltas
    }

    /// Flushes any remaining line at EOF and validates the HTTP status code.
    public mutating func finish() throws -> [String] {
        guard headersComplete, let code = statusCode else {
            throw LoopbackHTTPError.unexpectedEOF
        }
        let deltas = parseSSE ? drainLines(flushTail: true) : []
        compact(&body, offset: &bodyOffset)
        if code != 200 {
            throw LoopbackHTTPError.httpStatus(code, errorBody)
        }
        return deltas
    }

    private mutating func drainLines(flushTail: Bool) -> [String] {
        var out: [String] = []
        while bodyOffset < body.count {
            let lineEnd: Int
            let nextOffset: Int
            if let nl = findByte(0x0A, in: body, from: bodyOffset) {
                lineEnd = (nl > bodyOffset && body[nl - 1] == 0x0D) ? (nl - 1) : nl
                nextOffset = nl + 1
            } else if flushTail {
                lineEnd = (body.count > bodyOffset && body[body.count - 1] == 0x0D) ? (body.count - 1) : body.count
                nextOffset = body.count
            } else {
                break
            }

            let line = String(decoding: body[bodyOffset..<lineEnd], as: UTF8.self)
            bodyOffset = nextOffset

            if statusCode != 200 {
                if errorBody.count < 1024 {
                    errorBody += line
                    if errorBody.count > 1024 {
                        errorBody = String(errorBody.prefix(1024))
                    }
                }
                continue
            }

            guard line.hasPrefix("data:") else { continue }
            var payload = line.dropFirst(5)
            if payload.first == " " { payload = payload.dropFirst() }
            let trimmed = payload.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed == "[DONE]" {
                framing = .done
                isComplete = true
                break
            }
            guard let obj = try? JSONSerialization.jsonObject(with: Data(payload.utf8)) as? [String: Any],
                  let choices = obj["choices"] as? [[String: Any]],
                  let delta = choices.first?["delta"] as? [String: Any],
                  let text = delta["content"] as? String, !text.isEmpty else { continue }
            out.append(text)
        }
        return out
    }

    private func findHeaderEnd() -> (Int, Int)? {
        var i = rawOffset
        let n = raw.count
        while i + 1 < n {
            if raw[i] == 0x0D, i + 3 < n, raw[i + 1] == 0x0A, raw[i + 2] == 0x0D, raw[i + 3] == 0x0A {
                return (i, 4)
            }
            if raw[i] == 0x0A, raw[i + 1] == 0x0A {
                return (i, 2)
            }
            i += 1
        }
        return nil
    }

    private func findByte(_ target: UInt8, in buffer: [UInt8], from start: Int) -> Int? {
        var i = start
        while i < buffer.count {
            if buffer[i] == target { return i }
            i += 1
        }
        return nil
    }

    private func compact(_ buffer: inout [UInt8], offset: inout Int) {
        if offset == 0 { return }
        if offset >= buffer.count {
            buffer.removeAll(keepingCapacity: true)
            offset = 0
        } else if offset >= 4096 {
            buffer.removeFirst(offset)
            offset = 0
        }
    }
}
