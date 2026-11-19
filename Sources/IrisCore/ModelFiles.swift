import Foundation

/// Whether a downloaded model is all there, read from its Hugging Face cache folder
/// (`models--org--name`: `snapshots/<commit>/` holds links to `blobs/`).
///
/// A download in progress, or one that stopped, writes `blobs/<etag>.incomplete` and links
/// the file into the snapshot only once it's whole. So the model counts as on disk only when
/// a snapshot has every weight file: `model.safetensors`, or every file its
/// `model.safetensors.index.json` names, each non-empty. Counting bytes in the folder, as
/// before, called a download that stopped past halfway "downloaded".
public enum ModelFiles {
    public static func complete(in repoFolder: URL) -> Bool {
        let fm = FileManager.default
        let snapshots = repoFolder.appendingPathComponent("snapshots")
        guard let commits = try? fm.contentsOfDirectory(at: snapshots, includingPropertiesForKeys: nil) else { return false }
        return commits.contains { snapshot in
            weightFiles(in: snapshot).map { names in
                !names.isEmpty && names.allSatisfy { name in
                    // Follows the link into blobs/; a dangling link has no size.
                    let size = (try? fm.attributesOfItem(atPath: snapshot.appendingPathComponent(name).resolvingSymlinksInPath().path))?[.size] as? Int64
                    return (size ?? 0) > 0
                }
            } ?? false
        }
    }

    /// The weight files a snapshot needs, or nil when it has no weights at all.
    static func weightFiles(in snapshot: URL) -> [String]? {
        let index = snapshot.appendingPathComponent("model.safetensors.index.json")
        if let data = try? Data(contentsOf: index),
            let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let map = json["weight_map"] as? [String: String]
        {
            return Array(Set(map.values)).sorted()
        }
        let single = snapshot.appendingPathComponent("model.safetensors")
        return FileManager.default.fileExists(atPath: single.path) ? ["model.safetensors"] : nil
    }
}
