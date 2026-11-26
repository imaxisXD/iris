// Direct loopback TCP client for talking to llama-server on 127.0.0.1.
//
// Replaces FoundationNetworking.URLSession on Windows, where libcurl's multi-handle polling
// burned ~20 CPU seconds during a single 38 s SSE stream on a 2-vCPU VM — stealing half a core
// from llama-server. Blocking recv() on a dedicated background thread sleeps inside the OS kernel
// (~0% CPU) while waiting for tokens and wakes immediately when cancelled.
//
// Used on both Windows (WinSock2) and macOS (POSIX) so the exact HTTP/1.1 + chunked SSE code path
// runs identically during Mac development (`IRIS_ENGINE=llama`) and on Windows PCs.

import Foundation
import IrisCore
#if os(Windows)
import WinSDK
#elseif canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#endif

enum LoopbackClient {
    #if os(Windows)
    private typealias NativeSocket = SOCKET
    private static let invalidSocket: NativeSocket = ~SOCKET(0)
    private static let winsockReady: Bool = {
        var data = WSADATA()
        return WSAStartup(0x0202, &data) == 0
    }()
    #else
    private typealias NativeSocket = Int32
    private static let invalidSocket: NativeSocket = -1
    #endif

    /// Holds one open socket so cancellation from another thread can wake blocking connect/recv
    /// without ever closing the handle twice.
    private final class SocketBox: @unchecked Sendable {
        private let lock = NSLock()
        private var sock: NativeSocket = LoopbackClient.invalidSocket
        private var cancelled = false

        var isCancelled: Bool {
            lock.lock()
            defer { lock.unlock() }
            return cancelled
        }

        /// Stores a newly created socket, or closes it immediately if already cancelled.
        func attach(_ s: NativeSocket) -> Bool {
            lock.lock()
            if cancelled {
                lock.unlock()
                LoopbackClient.closeNative(s)
                return false
            }
            sock = s
            lock.unlock()
            return true
        }

        /// Wakes any blocked connect/recv. On Windows closesocket unblocks both connect and recv;
        /// on POSIX shutdown(SHUT_RDWR) wakes recv while letting the worker thread own close(fd).
        func cancel() {
            lock.lock()
            cancelled = true
            let s = sock
            #if os(Windows)
            sock = LoopbackClient.invalidSocket
            #endif
            lock.unlock()
            guard s != LoopbackClient.invalidSocket else { return }
            LoopbackClient.shutdownNative(s)
            #if os(Windows)
            LoopbackClient.closeNative(s)
            #endif
        }

        /// Closes the socket when the worker finishes normally (no-op if Windows cancel closed it).
        func finish() {
            lock.lock()
            let s = sock
            sock = LoopbackClient.invalidSocket
            lock.unlock()
            guard s != LoopbackClient.invalidSocket else { return }
            LoopbackClient.closeNative(s)
        }
    }

    private final class StatusOnce: @unchecked Sendable {
        private let lock = NSLock()
        private var continuation: CheckedContinuation<Int?, Never>?
        init(_ c: CheckedContinuation<Int?, Never>) { continuation = c }
        func resume(_ value: Int?) {
            lock.lock()
            let c = continuation
            continuation = nil
            lock.unlock()
            c?.resume(returning: value)
        }
    }

    /// Sends one loopback HTTP/1.1 request and returns its HTTP status code, or nil on error/timeout.
    static func status(
        port: Int,
        method: String,
        path: String,
        key: String,
        body: Data? = nil,
        within seconds: Double
    ) async -> Int? {
        var headers: [(String, String)] = [("Authorization", "Bearer \(key)")]
        if body != nil {
            headers.append(("Content-Type", "application/json"))
        }
        let wire = LoopbackHTTPRequest.encode(method: method, path: path, port: port, headers: headers, body: body)
        let box = SocketBox()

        return await withTaskCancellationHandler {
            await withCheckedContinuation { (c: CheckedContinuation<Int?, Never>) in
                let once = StatusOnce(c)
                let timer = DispatchWorkItem {
                    box.cancel()
                    once.resume(nil)
                }
                DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + seconds, execute: timer)

                Thread.detachNewThread {
                    defer {
                        timer.cancel()
                        box.finish()
                    }
                    guard let s = openAndConnect(port: port, timeout: seconds, box: box) else {
                        once.resume(nil)
                        return
                    }
                    guard sendAll(s, wire) else {
                        once.resume(nil)
                        return
                    }
                    var parser = LoopbackHTTPStream(parseSSE: false)
                    var buf = [CChar](repeating: 0, count: 4096)
                    while !box.isCancelled {
                        let n = recvChunk(s, into: &buf)
                        if n <= 0 { break }
                        let chunk = buf.withUnsafeBytes { Data(bytes: $0.baseAddress!, count: n) }
                        do {
                            _ = try parser.feed(chunk)
                        } catch {
                            break
                        }
                        if parser.headersComplete { break }
                    }
                    once.resume(parser.statusCode)
                }
            }
        } onCancel: {
            box.cancel()
        }
    }

    /// Streams OpenAI-compatible SSE `choices[0].delta.content` tokens from llama-server on loopback.
    static func stream(
        port: Int,
        path: String,
        key: String,
        body: Data
    ) -> AsyncThrowingStream<String, Error> {
        let wire = LoopbackHTTPRequest.encode(
            method: "POST",
            path: path,
            port: port,
            headers: [
                ("Authorization", "Bearer \(key)"),
                ("Content-Type", "application/json"),
                ("Accept", "text/event-stream"),
            ],
            body: body
        )

        return AsyncThrowingStream { continuation in
            let box = SocketBox()
            continuation.onTermination = { _ in
                box.cancel()
            }

            Thread.detachNewThread {
                defer { box.finish() }
                do {
                    guard let s = openAndConnect(port: port, timeout: 600, box: box) else {
                        if box.isCancelled {
                            continuation.finish()
                        } else {
                            continuation.finish(throwing: LlamaError.noServer("connect failed (\(lastSocketError()))"))
                        }
                        return
                    }
                    guard sendAll(s, wire) else {
                        if box.isCancelled {
                            continuation.finish()
                        } else {
                            continuation.finish(throwing: LlamaError.noServer("send failed (\(lastSocketError()))"))
                        }
                        return
                    }

                    var parser = LoopbackHTTPStream(parseSSE: true)
                    var buf = [CChar](repeating: 0, count: 16_384)
                    while !box.isCancelled {
                        let n = recvChunk(s, into: &buf)
                        if n == 0 { break }
                        if n < 0 {
                            if box.isCancelled {
                                continuation.finish()
                                return
                            }
                            throw LlamaError.noServer("recv failed (\(lastSocketError()))")
                        }
                        let chunk = buf.withUnsafeBytes { Data(bytes: $0.baseAddress!, count: n) }
                        let deltas = try parser.feed(chunk)
                        for text in deltas {
                            continuation.yield(text)
                        }
                        if parser.isComplete { break }
                    }

                    if box.isCancelled {
                        continuation.finish()
                        return
                    }
                    let tail = try parser.finish()
                    for text in tail {
                        continuation.yield(text)
                    }
                    continuation.finish()
                } catch LoopbackHTTPError.httpStatus(let code, let body) {
                    continuation.finish(throwing: LlamaError.noServer("HTTP \(code): \(body.prefix(200))"))
                } catch {
                    if box.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }
        }
    }

    // MARK: - Native socket helpers

    private static func openAndConnect(port: Int, timeout: Double, box: SocketBox) -> NativeSocket? {
        guard port > 0 && port <= 65_535 else { return nil }
        #if os(Windows)
        guard winsockReady else { return nil }
        let s = WinSDK.socket(Int32(AF_INET), Int32(SOCK_STREAM), 0)
        #else
        let s = DarwinOrPosix.socket(AF_INET, SOCK_STREAM, 0)
        #endif
        guard s != invalidSocket else { return nil }
        guard box.attach(s) else { return nil }

        configureSocket(s, timeout: timeout)

        var addr = sockaddr_in()
        #if os(Windows)
        addr.sin_family = UInt16(AF_INET)
        addr.sin_port = UInt16(port).bigEndian
        addr.sin_addr.S_un.S_addr = UInt32(0x7F00_0001).bigEndian
        let rc = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                WinSDK.connect(s, sa, Int32(MemoryLayout<sockaddr_in>.size))
            }
        }
        #else
        #if canImport(Darwin)
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        #endif
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = UInt16(port).bigEndian
        addr.sin_addr.s_addr = UInt32(0x7F00_0001).bigEndian
        let rc = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                DarwinOrPosix.connect(s, sa, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        #endif
        guard rc == 0 else { return nil }
        return s
    }

    private static func configureSocket(_ s: NativeSocket, timeout: Double) {
        #if os(Windows)
        var ms = DWORD(max(1, Int((timeout * 1000).rounded())))
        let optLen = Int32(MemoryLayout<DWORD>.size)
        withUnsafePointer(to: &ms) { ptr in
            ptr.withMemoryRebound(to: CChar.self, capacity: Int(optLen)) { p in
                _ = WinSDK.setsockopt(s, Int32(SOL_SOCKET), Int32(SO_RCVTIMEO), p, optLen)
                _ = WinSDK.setsockopt(s, Int32(SOL_SOCKET), Int32(SO_SNDTIMEO), p, optLen)
            }
        }
        #else
        #if canImport(Darwin)
        var noSigPipe: Int32 = 1
        _ = setsockopt(s, SOL_SOCKET, SO_NOSIGPIPE, &noSigPipe, socklen_t(MemoryLayout<Int32>.size))
        #endif
        let whole = Int(timeout)
        let usec = Int32(((timeout - Double(whole)) * 1_000_000).rounded())
        var tv = timeval(tv_sec: whole, tv_usec: usec)
        _ = setsockopt(s, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
        _ = setsockopt(s, SOL_SOCKET, SO_SNDTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
        #endif
    }

    private static func sendAll(_ s: NativeSocket, _ data: Data) -> Bool {
        data.withUnsafeBytes { raw -> Bool in
            guard let base = raw.baseAddress else { return true }
            let total = raw.count
            var sent = 0
            while sent < total {
                let remaining = total - sent
                #if os(Windows)
                let ptr = base.advanced(by: sent).assumingMemoryBound(to: CChar.self)
                let n = Int(WinSDK.send(s, ptr, Int32(min(remaining, 65_536)), 0))
                if n <= 0 { return false }
                #else
                let n = DarwinOrPosix.send(s, base.advanced(by: sent), remaining, 0)
                if n < 0 {
                    if errno == EINTR { continue }
                    return false
                }
                if n == 0 { return false }
                #endif
                sent += n
            }
            return true
        }
    }

    private static func recvChunk(_ s: NativeSocket, into buf: inout [CChar]) -> Int {
        while true {
            #if os(Windows)
            let n = Int(buf.withUnsafeMutableBufferPointer {
                WinSDK.recv(s, $0.baseAddress, Int32($0.count), 0)
            })
            return n
            #else
            let n = buf.withUnsafeMutableBytes {
                DarwinOrPosix.recv(s, $0.baseAddress, $0.count, 0)
            }
            if n < 0 && errno == EINTR { continue }
            return n
            #endif
        }
    }

    private static func shutdownNative(_ s: NativeSocket) {
        #if os(Windows)
        _ = WinSDK.shutdown(s, Int32(2)) // SD_BOTH
        #else
        _ = DarwinOrPosix.shutdown(s, SHUT_RDWR)
        #endif
    }

    private static func closeNative(_ s: NativeSocket) {
        #if os(Windows)
        _ = WinSDK.closesocket(s)
        #else
        _ = DarwinOrPosix.close(s)
        #endif
    }

    private static func lastSocketError() -> Int32 {
        #if os(Windows)
        return WSAGetLastError()
        #else
        return errno
        #endif
    }
}

#if !os(Windows)
private enum DarwinOrPosix {
    #if canImport(Darwin)
    static func socket(_ domain: Int32, _ type: Int32, _ proto: Int32) -> Int32 { Darwin.socket(domain, type, proto) }
    static func connect(_ fd: Int32, _ addr: UnsafePointer<sockaddr>?, _ len: socklen_t) -> Int32 { Darwin.connect(fd, addr, len) }
    static func send(_ fd: Int32, _ buf: UnsafeRawPointer?, _ len: Int, _ flags: Int32) -> Int { Darwin.send(fd, buf, len, flags) }
    static func recv(_ fd: Int32, _ buf: UnsafeMutableRawPointer?, _ len: Int, _ flags: Int32) -> Int { Darwin.recv(fd, buf, len, flags) }
    static func shutdown(_ fd: Int32, _ how: Int32) -> Int32 { Darwin.shutdown(fd, how) }
    static func close(_ fd: Int32) -> Int32 { Darwin.close(fd) }
    #elseif canImport(Glibc)
    static func socket(_ domain: Int32, _ type: Int32, _ proto: Int32) -> Int32 { Glibc.socket(domain, type, proto) }
    static func connect(_ fd: Int32, _ addr: UnsafePointer<sockaddr>?, _ len: socklen_t) -> Int32 { Glibc.connect(fd, addr, len) }
    static func send(_ fd: Int32, _ buf: UnsafeRawPointer?, _ len: Int, _ flags: Int32) -> Int { Glibc.send(fd, buf, len, flags) }
    static func recv(_ fd: Int32, _ buf: UnsafeMutableRawPointer?, _ len: Int, _ flags: Int32) -> Int { Glibc.recv(fd, buf, len, flags) }
    static func shutdown(_ fd: Int32, _ how: Int32) -> Int32 { Glibc.shutdown(fd, how) }
    static func close(_ fd: Int32) -> Int32 { Glibc.close(fd) }
    #endif
}
#endif
