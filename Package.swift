// swift-tools-version: 6.0

import PackageDescription

let package = Package(
  name: "EduToolsTests",
  platforms: [.macOS("26.0")],
  targets: [
    .target(
      name: "EduTools",
      path: "Sources/EduTools",
      exclude: ["main.swift"],
      swiftSettings: [.swiftLanguageMode(.v5)]
    ),
    .target(
      name: "TestSupport",
      dependencies: ["EduTools"],
      path: "Tests/Support",
      swiftSettings: [.swiftLanguageMode(.v5)]
    ),
    .testTarget(
      name: "UnitTests",
      dependencies: ["EduTools", "TestSupport"],
      path: "Tests/UnitTests",
      swiftSettings: [.swiftLanguageMode(.v5)]
    ),
    .testTarget(
      name: "IntegrationTests",
      dependencies: ["EduTools", "TestSupport"],
      path: "Tests/IntegrationTests",
      swiftSettings: [.swiftLanguageMode(.v5)]
    ),
    .testTarget(
      name: "GUITests",
      dependencies: ["EduTools", "TestSupport"],
      path: "Tests/GUITests",
      swiftSettings: [.swiftLanguageMode(.v5)]
    ),
  ],
  swiftLanguageModes: [.v5]
)
