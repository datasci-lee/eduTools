import Foundation
import Testing
import TestSupport

@testable import EduTools

@Suite("로컬 폴더 음악")
struct MusicPlaylistTests {
  @Test("수집은 확장자·숨김·깊이·상한을 지키고 숫자 이름순으로 정렬한다")
  func collectionFiltersBoundsAndNaturallySorts() throws {
    let root = URL(fileURLWithPath: "/library", isDirectory: true)
    let fileSystem = FakeMusicFileSystem(entries: [
      "/library": [
        .file("10.mp3"),
        .file("2.MP3"),
        .file("notes.txt"),
        .file(".hidden.wav"),
        .directory(".hidden-folder"),
        .directory("A"),
      ],
      "/library/A": [.directory("B"), .file("track.aac")],
      "/library/A/B": [.directory("C")],
      "/library/A/B/C": [.directory("D"), .file("inside.flac")],
      "/library/A/B/C/D": [.file("too-deep.wav")],
    ])

    let result = MusicPlaylistCollector(fileSystem: fileSystem).collect(root: root)

    #expect(result.failureReason == nil)
    #expect(result.tracks.map(\.relativePath) == [
      "2.MP3",
      "10.mp3",
      "A/B/C/inside.flac",
      "A/track.aac",
    ])

    let manyRoot = URL(fileURLWithPath: "/many", isDirectory: true)
    let manyEntries = (0...MusicPlaylistCollector.maximumTrackCount).map {
      FakeMusicFileSystem.Entry.file("track\($0).mp3")
    }
    let capped = MusicPlaylistCollector(
      fileSystem: FakeMusicFileSystem(entries: ["/many": manyEntries])
    ).collect(root: manyRoot)
    #expect(capped.tracks.count == MusicPlaylistCollector.maximumTrackCount)
    #expect(capped.tracks.last?.relativePath == "track499.mp3")

    let unreadable = MusicPlaylistCollector(
      fileSystem: FakeMusicFileSystem(entries: [:], failingPaths: ["/missing"])
    ).collect(root: URL(fileURLWithPath: "/missing", isDirectory: true))
    #expect(unreadable.tracks.isEmpty)
    #expect(unreadable.failureReason != nil)
  }

  @Test("순서기는 반복·건너뛰기·무작위 경계와 연속 실패를 구분한다")
  func playlistAdvancesAndStopsAfterConsecutiveFailures() throws {
    let tracks = ["10.mp3", "2.mp3", "3.mp3"].map {
      MusicTrack(url: URL(fileURLWithPath: "/music/\($0)"), relativePath: $0)
    }
    let named = MusicPlaylist(tracks: tracks, order: .name, repeat: .all)
    #expect(named.current?.relativePath == "2.mp3")
    #expect(named.advance(userSkip: false)?.relativePath == "3.mp3")
    #expect(named.advance(userSkip: false)?.relativePath == "10.mp3")
    #expect(named.advance(userSkip: false)?.relativePath == "2.mp3")

    let repeated = MusicPlaylist(tracks: tracks, order: .name, repeat: .one)
    let repeatedCurrent = try #require(repeated.current)
    #expect(repeated.advance(userSkip: false) == repeatedCurrent)
    #expect(repeated.advance(userSkip: true) != repeatedCurrent)

    let shuffled = MusicPlaylist(
      tracks: tracks,
      order: .shuffle,
      repeat: .all,
      rng: SeededRandom(seed: 0)
    )
    var previous = try #require(shuffled.current)
    for _ in 1..<tracks.count {
      previous = try #require(shuffled.advance(userSkip: false))
    }
    let wrapped = try #require(shuffled.advance(userSkip: false))
    #expect(wrapped != previous)

    let failures = MusicPlaylist(tracks: tracks, order: .name, repeat: .all)
    failures.reportFailure()
    failures.reportSuccess()
    #expect(failures.consecutiveFailures == 0)
    for _ in tracks {
      failures.reportFailure()
    }
    #expect(failures.exhausted)
  }
}

private struct FakeMusicFileSystem: MusicFileSystemListing {
  struct Entry {
    let name: String
    let isDirectory: Bool

    static func file(_ name: String) -> Entry {
      Entry(name: name, isDirectory: false)
    }

    static func directory(_ name: String) -> Entry {
      Entry(name: name, isDirectory: true)
    }
  }

  struct ReadError: Error {}

  let entries: [String: [Entry]]
  var failingPaths: Set<String> = []

  func contentsOfDirectory(at url: URL) throws -> [MusicFileEntry] {
    if failingPaths.contains(url.path) {
      throw ReadError()
    }
    return (entries[url.path] ?? []).map { entry in
      MusicFileEntry(
        url: url.appendingPathComponent(entry.name, isDirectory: entry.isDirectory),
        name: entry.name,
        isDirectory: entry.isDirectory,
        isRegularFile: !entry.isDirectory
      )
    }
  }
}
