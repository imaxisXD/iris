import Foundation
import Testing
@testable import IrisCore

@Suite struct LoopbackHTTPTests {
    private static func chunked(_ pieces: [Data]) -> Data {
        var out = Data()
        for piece in pieces {
            out.append(Data("\(String(piece.count, radix: 16))\r\n".utf8))
            out.append(piece)
            out.append(Data("\r\n".utf8))
        }
        out.append(Data("0\r\n\r\n".utf8))
        return out
    }

    @Test func encodesRequestWithHeadersAndBody() {
        let body = Data("{\"hello\":true}".utf8)
        let wire = LoopbackHTTPRequest.encode(
            method: "POST",
            path: "/v1/chat/completions",
            port: 54321,
            headers: [("Authorization", "Bearer secret"), ("Content-Type", "application/json")],
            body: body
        )
        let text = String(decoding: wire, as: UTF8.self)
        #expect(text.hasPrefix("POST /v1/chat/completions HTTP/1.1\r\n"))
        #expect(text.contains("Host: 127.0.0.1:54321\r\n"))
        #expect(text.contains("Connection: close\r\n"))
        #expect(text.contains("Authorization: Bearer secret\r\n"))
        #expect(text.contains("Content-Length: \(body.count)\r\n"))
        #expect(text.hasSuffix("\r\n\r\n{\"hello\":true}"))
    }

    @Test func parsesChunkedSSEByteByByte() throws {
        let sse1 = Data("data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n".utf8)
        let sse2 = Data("data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n".utf8)
        let sse3 = Data("data: {\"choices\":[{\"delta\":{}}]}\n\ndata: [DONE]\n\n".utf8)
        var wire = Data("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n".utf8)
        wire.append(Self.chunked([sse1, sse2, sse3]))

        var parser = LoopbackHTTPStream(parseSSE: true)
        var pieces: [String] = []
        for byte in wire {
            pieces.append(contentsOf: try parser.feed(Data([byte])))
        }
        pieces.append(contentsOf: try parser.finish())

        #expect(parser.statusCode == 200)
        #expect(parser.isComplete)
        #expect(pieces == ["Hello", " world"])
    }

    @Test func reassemblesMultiByteUTF8SplitAcrossHTTPChunks() throws {
        let fullLine = Data("data: {\"choices\":[{\"delta\":{\"content\":\"café 🎉\"}}]}\n\ndata: [DONE]\n\n".utf8)
        // Split right inside the 4-byte emoji sequence so neither HTTP chunk is valid UTF-8 alone.
        let emojiStart = fullLine.firstIndex(of: 0xF0)!
        let part1 = Data(fullLine[..<(emojiStart + 2)])
        let part2 = Data(fullLine[(emojiStart + 2)...])

        var wire = Data("HTTP/1.1 200 OK\r\nTransfer-Encoding: Chunked\r\n\r\n".utf8)
        wire.append(Self.chunked([part1, part2]))

        var parser = LoopbackHTTPStream(parseSSE: true)
        var pieces = try parser.feed(wire)
        pieces.append(contentsOf: try parser.finish())

        #expect(pieces == ["café 🎉"])
    }

    @Test func handlesChunkExtensionsAndContentLengthResponses() throws {
        var wire = Data("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".utf8)
        let payload = Data("data:{\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n".utf8)
        wire.append(Data("\(String(payload.count, radix: 16));ext=1\r\n".utf8))
        wire.append(payload)
        wire.append(Data("\r\n0;last=yes\r\n\r\n".utf8))

        var parser = LoopbackHTTPStream(parseSSE: true)
        var pieces = try parser.feed(wire)
        pieces.append(contentsOf: try parser.finish())
        #expect(pieces == ["hi"])

        let body = Data("data: {\"choices\":[{\"delta\":{\"content\":\"fixed\"}}]}\n".utf8)
        var clWire = Data("HTTP/1.1 200 OK\r\nContent-Length: \(body.count)\r\n\r\n".utf8)
        clWire.append(body)
        var clParser = LoopbackHTTPStream(parseSSE: true)
        var clPieces = try clParser.feed(clWire)
        #expect(clParser.isComplete)
        clPieces.append(contentsOf: try clParser.finish())
        #expect(clPieces == ["fixed"])
    }

    @Test func capturesNon200ErrorBodyWithoutTrailingNewline() throws {
        let errJson = "{\"error\":{\"message\":\"Loading model\"}}"
        let wire = Data("HTTP/1.1 503 Service Unavailable\r\nContent-Length: \(errJson.utf8.count)\r\n\r\n\(errJson)".utf8)

        var parser = LoopbackHTTPStream(parseSSE: true)
        let deltas = try parser.feed(wire)
        #expect(deltas.isEmpty)
        #expect(parser.statusCode == 503)
        #expect(parser.isComplete)

        do {
            _ = try parser.finish()
            Issue.record("expected httpStatus error")
        } catch let LoopbackHTTPError.httpStatus(code, body) {
            #expect(code == 503)
            #expect(body == errJson)
        }
    }

    @Test func statusOnlyModeStopsAtHeaders() throws {
        let wire = Data("HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\n{\"status\":1}".utf8)
        var parser = LoopbackHTTPStream(parseSSE: false)
        _ = try parser.feed(wire)
        #expect(parser.headersComplete)
        #expect(parser.isComplete)
        #expect(parser.statusCode == 200)
    }

    @Test func skipsInterim100ContinueHeader() throws {
        let sse = Data("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n".utf8)
        var wire = Data("HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".utf8)
        wire.append(Self.chunked([sse]))

        var parser = LoopbackHTTPStream(parseSSE: true)
        var pieces = try parser.feed(wire)
        pieces.append(contentsOf: try parser.finish())
        #expect(parser.statusCode == 200)
        #expect(pieces == ["ok"])
    }

    @Test func rejectsMalformedHeadersAndTruncatedStream() throws {
        var bad = LoopbackHTTPStream(parseSSE: true)
        #expect(throws: LoopbackHTTPError.malformedHeaders) {
            _ = try bad.feed(Data("NOT_HTTP\r\n\r\n".utf8))
        }

        var empty = LoopbackHTTPStream(parseSSE: true)
        #expect(throws: LoopbackHTTPError.unexpectedEOF) {
            _ = try empty.finish()
        }
    }
}
