import Foundation

enum BreakMusicSource: String, Codable, CaseIterable {
    case youtube
    case folder
}

enum BreakMusicOrder: String, Codable, CaseIterable {
    case name
    case shuffle
}

enum BreakMusicRepeatMode: String, Codable, CaseIterable {
    case all
    case one
}

struct MusicTrack: Equatable {
    let url: URL
    let relativePath: String
}

struct MusicFileEntry {
    let url: URL
    let name: String
    let isDirectory: Bool
    let isRegularFile: Bool
}

protocol MusicFileSystemListing {
    func contentsOfDirectory(at url: URL) throws -> [MusicFileEntry]
}

struct LocalMusicFileSystem: MusicFileSystemListing {
    private let fileManager: FileManager

    init(fileManager: FileManager = .default) {
        self.fileManager = fileManager
    }

    func contentsOfDirectory(at url: URL) throws -> [MusicFileEntry] {
        let keys: Set<URLResourceKey> = [.isDirectoryKey, .isRegularFileKey, .isSymbolicLinkKey]
        return try fileManager.contentsOfDirectory(
            at: url,
            includingPropertiesForKeys: Array(keys),
            options: []
        ).map { entryURL in
            let values = try entryURL.resourceValues(forKeys: keys)
            let isSymbolicLink = values.isSymbolicLink == true
            return MusicFileEntry(
                url: entryURL,
                name: entryURL.lastPathComponent,
                isDirectory: values.isDirectory == true && !isSymbolicLink,
                isRegularFile: values.isRegularFile == true && !isSymbolicLink
            )
        }
    }
}

struct MusicCollectionResult {
    let tracks: [MusicTrack]
    let failureReason: String?
}

enum MusicTrackPathOrder {
    // Swift: natural relative-path order / Rust: natural_relative_path_order
    static func areInIncreasingOrder(_ lhs: String, _ rhs: String) -> Bool {
        let options: String.CompareOptions = [.caseInsensitive, .numeric]
        let comparison = lhs.compare(
            rhs,
            options: options,
            range: nil,
            locale: Locale(identifier: "en_US_POSIX")
        )
        if comparison != .orderedSame {
            return comparison == .orderedAscending
        }
        return lhs.compare(rhs, options: .literal) == .orderedAscending
    }
}

struct MusicPlaylistCollector {
    static let maximumDepth = 3
    static let maximumTrackCount = 500
    static let supportedExtensions: Set<String> = ["mp3", "m4a", "aac", "wav", "flac"]

    private let fileSystem: MusicFileSystemListing

    init(fileSystem: MusicFileSystemListing = LocalMusicFileSystem()) {
        self.fileSystem = fileSystem
    }

    // Swift: collect(root:) / Rust: collect(root)
    func collect(root: URL) -> MusicCollectionResult {
        do {
            var tracks: [MusicTrack] = []
            try collect(
                directory: root,
                relativeDirectory: "",
                depth: 0,
                tracks: &tracks
            )
            return MusicCollectionResult(
                tracks: tracks.sorted {
                    MusicTrackPathOrder.areInIncreasingOrder($0.relativePath, $1.relativePath)
                },
                failureReason: nil
            )
        } catch {
            let reason = String(describing: error)
            NSLog(
                "EduTools: break music folder could not be read at %@: %@",
                root.path,
                reason
            )
            return MusicCollectionResult(tracks: [], failureReason: reason)
        }
    }

    private func collect(
        directory: URL,
        relativeDirectory: String,
        depth: Int,
        tracks: inout [MusicTrack]
    ) throws {
        guard tracks.count < Self.maximumTrackCount else { return }
        let entries = try fileSystem.contentsOfDirectory(at: directory).sorted {
            MusicTrackPathOrder.areInIncreasingOrder($0.name, $1.name)
        }
        for entry in entries {
            guard tracks.count < Self.maximumTrackCount else { return }
            guard !entry.name.hasPrefix(".") else { continue }
            let relativePath = relativeDirectory.isEmpty
                ? entry.name
                : "\(relativeDirectory)/\(entry.name)"
            if entry.isDirectory {
                if depth < Self.maximumDepth {
                    try collect(
                        directory: entry.url,
                        relativeDirectory: relativePath,
                        depth: depth + 1,
                        tracks: &tracks
                    )
                }
                continue
            }
            guard entry.isRegularFile,
                  Self.supportedExtensions.contains(entry.url.pathExtension.lowercased())
            else {
                continue
            }
            tracks.append(MusicTrack(url: entry.url, relativePath: relativePath))
        }
    }
}

// Swift: MusicPlaylist / Rust: Playlist
final class MusicPlaylist {
    let order: BreakMusicOrder
    let repeatMode: BreakMusicRepeatMode
    private(set) var current: MusicTrack?
    private(set) var consecutiveFailures = 0
    private(set) var exhausted = false

    private let tracks: [MusicTrack]
    private let randomValue: () -> UInt64
    private var playOrder: [MusicTrack]
    private var currentIndex = 0

    convenience init<R: RandomNumberGenerator>(
        tracks: [MusicTrack],
        order: BreakMusicOrder,
        repeat repeatMode: BreakMusicRepeatMode,
        rng: R
    ) {
        var generator = rng
        self.init(
            tracks: tracks,
            order: order,
            repeat: repeatMode,
            randomValue: { generator.next() }
        )
    }

    convenience init(
        tracks: [MusicTrack],
        order: BreakMusicOrder,
        repeat repeatMode: BreakMusicRepeatMode
    ) {
        self.init(
            tracks: tracks,
            order: order,
            repeat: repeatMode,
            rng: SystemRandomNumberGenerator()
        )
    }

    private init(
        tracks: [MusicTrack],
        order: BreakMusicOrder,
        repeat repeatMode: BreakMusicRepeatMode,
        randomValue: @escaping () -> UInt64
    ) {
        self.order = order
        self.repeatMode = repeatMode
        self.tracks = tracks
        self.randomValue = randomValue
        switch order {
        case .name:
            playOrder = tracks.sorted {
                MusicTrackPathOrder.areInIncreasingOrder($0.relativePath, $1.relativePath)
            }
        case .shuffle:
            playOrder = tracks
            Self.shuffle(&playOrder, randomValue: randomValue)
        }
        current = playOrder.first
    }

    // Swift: advance(userSkip:) / Rust: advance(user_skip)
    @discardableResult
    func advance(userSkip: Bool) -> MusicTrack? {
        guard !playOrder.isEmpty, !exhausted else {
            current = nil
            return nil
        }
        if repeatMode == .one, !userSkip {
            return current
        }
        if currentIndex + 1 < playOrder.count {
            currentIndex += 1
        } else {
            let previous = current
            currentIndex = 0
            if order == .shuffle {
                Self.shuffle(&playOrder, randomValue: randomValue)
                if playOrder.count >= 2, playOrder.first == previous {
                    playOrder.swapAt(0, 1)
                }
            }
        }
        current = playOrder[currentIndex]
        return current
    }

    func reportFailure() {
        guard !tracks.isEmpty else { return }
        consecutiveFailures += 1
        exhausted = consecutiveFailures >= tracks.count
    }

    func reportSuccess() {
        consecutiveFailures = 0
        exhausted = false
    }

    private static func shuffle(_ values: inout [MusicTrack], randomValue: () -> UInt64) {
        guard values.count >= 2 else { return }
        for index in stride(from: values.count - 1, through: 1, by: -1) {
            let swapIndex = Int(randomValue() % UInt64(index + 1))
            values.swapAt(index, swapIndex)
        }
    }
}
