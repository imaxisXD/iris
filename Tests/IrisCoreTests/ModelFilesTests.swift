import Foundation
import Testing
@testable import IrisCore

@Suite struct ModelFilesTests {
    private func repo() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("models--test--\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: url.appendingPathComponent("blobs"), withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: url.appendingPathComponent("snapshots/abc"), withIntermediateDirectories: true)
        return url
    }

    private func link(_ repo: URL, _ name: String, bytes: Int) throws {
        let blob = repo.appendingPathComponent("blobs/\(name)-blob")
        try Data(repeating: 1, count: bytes).write(to: blob)
        try FileManager.default.createSymbolicLink(at: repo.appendingPathComponent("snapshots/abc/\(name)"), withDestinationURL: blob)
    }

    /// The bug: a download that stopped past halfway left a big `.incomplete` blob, and the
    /// byte count called the model downloaded.
    @Test func aStoppedDownloadIsNotOnDisk() throws {
        let r = try repo()
        try Data(repeating: 1, count: 4096).write(to: r.appendingPathComponent("blobs/etag.incomplete"))
        try link(r, "config.json", bytes: 10)
        #expect(!ModelFiles.complete(in: r))
    }

    @Test func aSingleWeightFileLinkedIsOnDisk() throws {
        let r = try repo()
        try link(r, "model.safetensors", bytes: 10)
        #expect(ModelFiles.complete(in: r))
    }

    @Test func shardedWeightsNeedEveryShard() throws {
        let r = try repo()
        let index = #"{"weight_map":{"a":"model-00001.safetensors","b":"model-00002.safetensors"}}"#
        try Data(index.utf8).write(to: r.appendingPathComponent("snapshots/abc/model.safetensors.index.json"))
        try link(r, "model-00001.safetensors", bytes: 10)
        #expect(!ModelFiles.complete(in: r))
        try link(r, "model-00002.safetensors", bytes: 10)
        #expect(ModelFiles.complete(in: r))
    }

    @Test func noFolderIsNotOnDisk() {
        #expect(!ModelFiles.complete(in: URL(fileURLWithPath: "/nonexistent/models--x")))
    }
}

