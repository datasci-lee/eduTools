import Foundation

public enum FixtureError: Error, CustomStringConvertible {
  case notFound(relativePath: String, searchedPaths: [String])
  case unreadable(path: String, reason: String)
  case decodingFailed(path: String, reason: String)

  public var description: String {
    switch self {
    case .notFound(let relativePath, let searchedPaths):
      return
        "JSON fixture '\(relativePath)' was not found. Searched: \(searchedPaths.joined(separator: ", "))"
    case .unreadable(let path, let reason):
      return "JSON fixture at '\(path)' could not be read: \(reason)"
    case .decodingFailed(let path, let reason):
      return "JSON fixture at '\(path)' could not be decoded: \(reason)"
    }
  }
}

public struct ShortcutFixtureBinding {
  public let definitionID: String
  public let scope: String
  public let portableString: String

  public init(definitionID: String, scope: String, portableString: String) {
    self.definitionID = definitionID
    self.scope = scope
    self.portableString = portableString
  }
}

public struct ShortcutDefaultsFixture: Codable, Equatable {
  public let formatVersion: Int
  public let bindings: [String: String]
  public let unbound: [String]

  public init(formatVersion: Int, bindings: [String: String], unbound: [String]) {
    self.formatVersion = formatVersion
    self.bindings = bindings
    self.unbound = unbound
  }
}

public enum Fixtures {
  private static let repositoryRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .deletingLastPathComponent()

  private static let searchRoots = [
    repositoryRoot.appendingPathComponent("fixtures", isDirectory: true),
    repositoryRoot.appendingPathComponent("Tests/Fixtures", isDirectory: true),
  ]

  public static func jsonData(named relativePath: String) throws -> Data {
    let candidates = searchRoots.map { root in
      root.appendingPathComponent(relativePath, isDirectory: false)
    }
    guard
      let fixtureURL = candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) })
    else {
      throw FixtureError.notFound(
        relativePath: relativePath,
        searchedPaths: candidates.map(\.path)
      )
    }
    do {
      return try Data(contentsOf: fixtureURL)
    } catch {
      throw FixtureError.unreadable(path: fixtureURL.path, reason: String(describing: error))
    }
  }

  public static func decode<T: Decodable>(
    _ type: T.Type,
    from relativePath: String,
    using decoder: JSONDecoder = JSONDecoder()
  ) throws -> T {
    let data = try jsonData(named: relativePath)
    do {
      return try decoder.decode(type, from: data)
    } catch {
      throw FixtureError.decodingFailed(path: relativePath, reason: String(describing: error))
    }
  }

  public static func shortcutDefaults(
    bindings: [ShortcutFixtureBinding],
    unbound: [String]
  ) -> ShortcutDefaultsFixture {
    ShortcutDefaultsFixture(
      formatVersion: 1,
      bindings: Dictionary(uniqueKeysWithValues: bindings.map {
        ("\($0.definitionID)|\($0.scope)", $0.portableString)
      }),
      unbound: unbound.sorted()
    )
  }
}
