import Foundation

public final class TempDir {
  public let url: URL

  private let fileManager: FileManager

  public init(fileManager: FileManager = .default) throws {
    self.fileManager = fileManager
    url = fileManager.temporaryDirectory
      .appendingPathComponent("EduToolsTests-\(UUID().uuidString)", isDirectory: true)
    try fileManager.createDirectory(at: url, withIntermediateDirectories: false)
  }

  deinit {
    guard fileManager.fileExists(atPath: url.path) else { return }
    do {
      try fileManager.removeItem(at: url)
    } catch {
      let message = "TempDir cleanup failed at '\(url.path)': \(error)\n"
      FileHandle.standardError.write(Data(message.utf8))
    }
  }
}
