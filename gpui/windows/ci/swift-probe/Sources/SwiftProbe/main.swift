import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

var failures = 0
func check(_ name: String, _ ok: Bool, _ detail: String) {
    print("\(ok ? "PASS" : "FAIL") \(name): \(detail)")
    if !ok { failures += 1 }
}

// 1. Memory: picks Lighter or Best.
let gb = Double(ProcessInfo.processInfo.physicalMemory) / 1_073_741_824
check("memory", gb > 1, String(format: "%.1f GB", gb))

// 2. A child process: started, its output read, a long one stopped.
do {
    let p = Process()
    p.executableURL = URL(fileURLWithPath: "C:\\Windows\\System32\\cmd.exe")
    p.arguments = ["/c", "echo", "iris-child"]
    let pipe = Pipe()
    p.standardOutput = pipe
    try p.run()
    p.waitUntilExit()
    let out = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
    check("process output", out.contains("iris-child") && p.terminationStatus == 0, out.trimmingCharacters(in: .whitespacesAndNewlines))
    let long = Process()
    long.executableURL = URL(fileURLWithPath: "C:\\Windows\\System32\\ping.exe")
    long.arguments = ["-n", "60", "127.0.0.1"]
    long.standardOutput = FileHandle.nullDevice
    try long.run()
    Thread.sleep(forTimeInterval: 1)
    let started = Date()
    long.terminate()
    long.waitUntilExit()
    check("process stop", Date().timeIntervalSince(started) < 5, "stopped in \(String(format: "%.2f", Date().timeIntervalSince(started))) s")
} catch { check("process", false, "\(error)") }

// 3. Streaming: an answer read piece by piece as it arrives (llama-server's server-sent events).
// URLSession.bytes isn't in Foundation on Windows (measured), so the helper streams through a
// data delegate, which both systems have.
final class Stream: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    var pieces = 0
    var bytes = 0
    var status = -1
    var error: Error?
    let finished = DispatchSemaphore(value: 0)
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        status = (response as? HTTPURLResponse)?.statusCode ?? -1
        completionHandler(.allow)
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        pieces += 1
        bytes += data.count
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        self.error = error
        finished.signal()
    }
}
let stream = Stream()
let streamSession = URLSession(configuration: .default, delegate: stream, delegateQueue: nil)
streamSession.dataTask(with: URL(string: "https://huggingface.co/ImAXISOG/iris-gemma-4-e2b-it-gguf/resolve/main/README.md")!).resume()
stream.finished.wait()
check("streaming through a data delegate", stream.error == nil && stream.status == 200 && stream.bytes > 0,
      "\(stream.bytes) bytes in \(stream.pieces) pieces, status \(stream.status), error \(String(describing: stream.error))")

// 4. A large download with progress (the model files are 2.8 and 4.6 GB): 25 MB of llama.cpp.
final class Progress: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    var reports = 0
    var done: URL?
    var error: Error?
    let finished = DispatchSemaphore(value: 0)
    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData _: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        reports += 1
    }
    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        let keep = FileManager.default.temporaryDirectory.appendingPathComponent("probe-download.zip")
        try? FileManager.default.removeItem(at: keep)
        do { try FileManager.default.moveItem(at: location, to: keep); done = keep } catch { self.error = error }
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let error { self.error = error }
        finished.signal()
    }
}
let progress = Progress()
let session = URLSession(configuration: .default, delegate: progress, delegateQueue: nil)
let started = Date()
session.downloadTask(with: URL(string: "https://github.com/ggml-org/llama.cpp/releases/download/b12110/llama-b12110-bin-win-cpu-x64.zip")!).resume()
progress.finished.wait()
let size = (progress.done.flatMap { try? FileManager.default.attributesOfItem(atPath: $0.path)[.size] as? Int }) ?? 0
check("download with progress", progress.error == nil && size == 24_708_445 && progress.reports > 1,
      "\(size) bytes, \(progress.reports) progress reports, \(String(format: "%.1f", Date().timeIntervalSince(started))) s, error \(String(describing: progress.error))")

// 5. Free disk space, for "no room" before a download.
do {
    let values = try URL(fileURLWithPath: NSHomeDirectory()).resourceValues(forKeys: [.volumeAvailableCapacityKey])
    check("free disk space", (values.volumeAvailableCapacity ?? 0) > 0, "\((values.volumeAvailableCapacity ?? 0) / 1_073_741_824) GB")
} catch { check("free disk space", false, "\(error)") }

print(failures == 0 ? "all passed" : "\(failures) failed")
exit(failures == 0 ? 0 : 1)
