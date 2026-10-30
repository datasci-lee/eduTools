import Foundation
import TestSupport
import Testing

@testable import EduTools

enum DisplayGridVectorError: Error {
  case unsupportedVersion(Int)
}

struct DisplayGridVectorFile: Decodable, Sendable {
  let version: Int
  let cases: [DisplayGridVectorCase]
}

struct DisplayGridVectorCase: Decodable, Sendable {
  let name: String
  let input: Input
  let expected: Expected

  struct Input: Decodable, Sendable {
    let metrics: [Metric]
  }

  struct Metric: Decodable, Sendable {
    let cell: Cell
    let size: Size
  }

  struct Cell: Decodable, Sendable {
    let x: Int
    let y: Int

    var gridCell: GridCell {
      GridCell(x: x, y: y)
    }
  }

  struct Size: Decodable, Sendable {
    let width: Double
    let height: Double

    var displaySize: DisplaySize {
      DisplaySize(width: width, height: height)
    }
  }

  struct Expected: Decodable, Sendable {
    let origins: [Origin]
  }

  struct Origin: Decodable, Sendable {
    let cell: Cell
    let origin: Point
  }

  struct Point: Decodable, Sendable {
    let x: Double
    let y: Double

    var displayPoint: DisplayPoint {
      DisplayPoint(x: x, y: y)
    }
  }
}

private func displayGridCases() throws -> [DisplayGridVectorCase] {
  let fixture = try Fixtures.decode(
    DisplayGridVectorFile.self,
    from: "vectors/display-grid-layouts.json"
  )
  guard fixture.version == 1 else {
    throw DisplayGridVectorError.unsupportedVersion(fixture.version)
  }
  return fixture.cases
}

private func displayFixture(
  id: UInt32,
  size: DisplaySize,
  kind: DisplayKind = .external
) -> (online: OnlineDisplay, monitor: PresetMonitor) {
  let online = OnlineDisplay(
    id: id,
    uuid: "unit-test-display-\(id)",
    vendor: 1_000 + id,
    model: 2_000 + id,
    serial: 3_000 + id,
    name: "Unit Test Display \(id)",
    kind: kind,
    size: size,
    origin: .zero,
    currentMirrorMasterID: nil
  )
  return (online, PresetMonitor(identity: online.identity))
}

private struct PlannedRectangle {
  let minimumX: Double
  let maximumX: Double
  let minimumY: Double
  let maximumY: Double

  init(screen: DisplayPlannedScreen) {
    minimumX = screen.origin.x
    maximumX = screen.origin.x + screen.size.width
    minimumY = screen.origin.y
    maximumY = screen.origin.y + screen.size.height
  }

  func overlaps(_ other: PlannedRectangle) -> Bool {
    min(maximumX, other.maximumX) > max(minimumX, other.minimumX)
      && min(maximumY, other.maximumY) > max(minimumY, other.minimumY)
  }
}

enum DisplayAutoSwitchBoundary: CaseIterable, Sendable {
  case unchangedConnectionSet
  case disabled
  case noMatchingPreset
  case oneExtraOnlineDisplay
  case oneMissingOnlineDisplay
  case noAutomaticMonitors
  case firstMatchingPresetWins
  case currentLayoutAlreadyMatches
  case insideCooldown
  case cooldownElapsed
}

@Suite("화면 구성 플래너")
struct DisplayPlannerTests {
  @Test(
    "격자 셀과 모니터 크기는 원점에서 변을 맞대는 배치로 계산된다",
    arguments: try displayGridCases()
  )
  func gridLayoutsFollowVector(_ testCase: DisplayGridVectorCase) throws {
    let metrics = testCase.input.metrics.map {
      DisplayGridMetric(cell: $0.cell.gridCell, size: $0.size.displaySize)
    }
    let displays = metrics.enumerated().map { index, metric in
      displayFixture(id: UInt32(index + 1), size: metric.size)
    }
    let screens = zip(displays, metrics).map { display, metric in
      PresetScreen(monitors: [display.monitor], cell: metric.cell)
    }
    let plan = DisplayPlanner.plan(
      preset: DisplayPreset(
        name: testCase.name,
        main: try #require(screens.first),
        secondary1: screens.indices.contains(1) ? screens[1] : nil,
        secondary2: screens.indices.contains(2) ? screens[2] : nil
      ),
      online: displays.map(\.online)
    )
    let origins = Dictionary(uniqueKeysWithValues: plan.screens.map { ($0.cell, $0.origin) })

    #expect(origins.count == metrics.count, "case=\(testCase.name)")
    for expected in testCase.expected.origins {
      #expect(
        origins[expected.cell.gridCell] == expected.origin.displayPoint,
        "case=\(testCase.name), cell=(\(expected.cell.x),\(expected.cell.y))"
      )
    }
  }

  @Test("연결된 첫 화면은 원점의 주 디스플레이가 되고 겹치는 계획은 거부된다")
  func connectedMainAnchorsAtZeroAndOverlapIsRejected() {
    let absentMain = displayFixture(
      id: 21,
      size: DisplaySize(width: 1920, height: 1080)
    )
    let connectedLeft = displayFixture(
      id: 22,
      size: DisplaySize(width: 2560, height: 1440)
    )
    let connectedRight = displayFixture(
      id: 23,
      size: DisplaySize(width: 1280, height: 720)
    )
    let missingMainPlan = DisplayPlanner.plan(
      preset: DisplayPreset(
        name: "Missing main",
        main: PresetScreen(monitors: [absentMain.monitor]),
        secondary1: PresetScreen(
          monitors: [connectedLeft.monitor],
          cell: GridCell(x: -1, y: 0)
        ),
        secondary2: PresetScreen(
          monitors: [connectedRight.monitor],
          cell: GridCell(x: 1, y: 0)
        )
      ),
      online: [connectedLeft.online, connectedRight.online]
    )
    let overlapMain = displayFixture(
      id: 24,
      size: DisplaySize(width: 1920, height: 1080)
    )
    let overlapRight = displayFixture(
      id: 25,
      size: DisplaySize(width: 1280, height: 1440)
    )
    let overlapBelow = displayFixture(
      id: 26,
      size: DisplaySize(width: 3000, height: 720)
    )
    let overlapPlan = DisplayPlanner.plan(
      preset: DisplayPreset(
        name: "Overlap",
        main: PresetScreen(monitors: [overlapMain.monitor]),
        secondary1: PresetScreen(
          monitors: [overlapRight.monitor],
          cell: GridCell(x: 1, y: 0)
        ),
        secondary2: PresetScreen(
          monitors: [overlapBelow.monitor],
          cell: GridCell(x: 0, y: 1)
        )
      ),
      online: [overlapMain.online, overlapRight.online, overlapBelow.online]
    )

    #expect(missingMainPlan.mainDisplayID == connectedLeft.online.id)
    #expect(missingMainPlan.originsByMaster[connectedLeft.online.id] == .zero)
    #expect(
      missingMainPlan.originsByMaster[connectedRight.online.id]
        == DisplayPoint(x: 2560, y: 0)
    )
    #expect(!overlapPlan.layoutIssues.isEmpty)
    #expect(!overlapPlan.isApplicable)
  }

  @Test("미러·마스터 승계·누락·프리셋 밖 모니터 규칙을 보존한다")
  func mirrorSuccessionMissingAndOutsideRulesArePreserved() {
    let displayOne = displayFixture(
      id: 1,
      size: DisplaySize(width: 2560, height: 1440)
    )
    let displayTwo = displayFixture(
      id: 2,
      size: DisplaySize(width: 1920, height: 1080)
    )
    let displayThree = displayFixture(
      id: 3,
      size: DisplaySize(width: 1280, height: 720)
    )
    let mirrorPlan = DisplayPlanner.plan(
      preset: DisplayPreset(
        name: "Mirrored main",
        main: PresetScreen(
          monitors: [displayOne.monitor, displayTwo.monitor, displayThree.monitor]
        )
      ),
      online: [displayOne.online, displayTwo.online, displayThree.online]
    )
    let extendedMirrorPlan = DisplayPlanner.plan(
      preset: DisplayPreset(
        name: "Mirrored secondary",
        main: PresetScreen(monitors: [displayOne.monitor]),
        secondary1: PresetScreen(
          monitors: [displayTwo.monitor, displayThree.monitor],
          cell: GridCell(x: 1, y: 0)
        )
      ),
      online: [displayOne.online, displayTwo.online, displayThree.online]
    )
    let missingFirst = displayFixture(
      id: 31,
      size: DisplaySize(width: 1920, height: 1080)
    )
    let succeedingMaster = displayFixture(
      id: 32,
      size: DisplaySize(width: 1920, height: 1080)
    )
    let successionPlan = DisplayPlanner.plan(
      preset: DisplayPreset(
        name: "Master succession",
        main: PresetScreen(monitors: [missingFirst.monitor, succeedingMaster.monitor])
      ),
      online: [succeedingMaster.online]
    )
    let outsideDisplay = displayFixture(
      id: 33,
      size: DisplaySize(width: 1024, height: 768),
      kind: .virtual
    )
    let outsidePlan = DisplayPlanner.plan(
      preset: DisplayPreset(
        name: "Outside display",
        main: PresetScreen(monitors: [displayOne.monitor])
      ),
      online: [displayOne.online, outsideDisplay.online]
    )

    #expect(mirrorPlan.mainDisplayID == displayOne.online.id)
    #expect(
      mirrorPlan.mirrors == [
        DisplayMirrorPlan(slaveID: displayTwo.online.id, masterID: displayOne.online.id),
        DisplayMirrorPlan(slaveID: displayThree.online.id, masterID: displayOne.online.id),
      ]
    )
    #expect(
      extendedMirrorPlan.mirrors == [
        DisplayMirrorPlan(slaveID: displayThree.online.id, masterID: displayTwo.online.id)
      ]
    )
    #expect(
      extendedMirrorPlan.originsByMaster[displayTwo.online.id]
        == DisplayPoint(x: 2560, y: 0)
    )
    #expect(successionPlan.mainDisplayID == succeedingMaster.online.id)
    #expect(successionPlan.missingPresetMonitors == [missingFirst.monitor.identity])
    #expect(outsidePlan.outsidePlacements.count == 1)
    #expect(outsidePlan.outsidePlacements.first?.displayID == outsideDisplay.online.id)
    #expect((outsidePlan.outsidePlacements.first?.origin.x ?? -1) >= 2560)
  }

  @Test("격자는 중복·대각선 연결을 거부하고 변으로 이어진 셀을 허용한다")
  func gridValidationRejectsDuplicateAndDiagonalCells() {
    let displayOne = displayFixture(
      id: 1,
      size: DisplaySize(width: 2560, height: 1440)
    )
    let displayTwo = displayFixture(
      id: 2,
      size: DisplaySize(width: 1920, height: 1080)
    )
    let displayThree = displayFixture(
      id: 3,
      size: DisplaySize(width: 1280, height: 720)
    )
    let duplicate = DisplayPreset(
      name: "Duplicate cell",
      main: PresetScreen(monitors: [displayOne.monitor]),
      secondary1: PresetScreen(monitors: [displayTwo.monitor])
    )
    let diagonal = DisplayPreset(
      name: "Diagonal",
      main: PresetScreen(monitors: [displayOne.monitor]),
      secondary1: PresetScreen(
        monitors: [displayTwo.monitor],
        cell: GridCell(x: 1, y: 1)
      )
    )
    let connected = DisplayPreset(
      name: "Connected",
      main: PresetScreen(monitors: [displayOne.monitor]),
      secondary1: PresetScreen(
        monitors: [displayTwo.monitor],
        cell: GridCell(x: 1, y: 0)
      ),
      secondary2: PresetScreen(
        monitors: [displayThree.monitor],
        cell: GridCell(x: 1, y: 1)
      )
    )

    #expect(DisplayGrid.validationError(for: duplicate) == .duplicateGridCell)
    #expect(DisplayGrid.validationError(for: diagonal) == .disconnectedGrid)
    #expect(DisplayGrid.validationError(for: connected) == nil)
  }

  @Test("자동 프리셋은 정확한 연결 집합만 고르고 배열의 앞선 항목을 우선한다")
  func automaticSelectionRequiresExactSetAndPreservesPriority() throws {
    let displayOne = displayFixture(
      id: 1,
      size: DisplaySize(width: 2560, height: 1440)
    )
    let displayTwo = displayFixture(
      id: 2,
      size: DisplaySize(width: 1920, height: 1080)
    )
    let displayThree = displayFixture(
      id: 3,
      size: DisplaySize(width: 1280, height: 720)
    )
    var monitorOne = displayOne.monitor
    monitorOne.autoMatch = true
    var monitorTwo = displayTwo.monitor
    monitorTwo.autoMatch = true
    let presetA = DisplayPreset(
      id: try #require(UUID(uuidString: "00000000-0000-0000-0000-00000000000A")),
      name: "Automatic A",
      main: PresetScreen(monitors: [monitorOne, monitorTwo])
    )
    let presetB = DisplayPreset(
      id: try #require(UUID(uuidString: "00000000-0000-0000-0000-00000000000B")),
      name: "Automatic B",
      main: PresetScreen(monitors: [monitorOne, monitorTwo])
    )
    let noAutomaticMonitors = DisplayPreset(
      name: "No automatic monitors",
      main: PresetScreen(monitors: [displayOne.monitor])
    )

    #expect(
      DisplayPlanner.presetForConnectedSet(
        connected: [displayOne.online, displayTwo.online],
        presets: [presetA, presetB]
      )?.id == presetA.id
    )
    #expect(
      DisplayPlanner.presetForConnectedSet(
        connected: [displayOne.online, displayTwo.online],
        presets: [presetB, presetA]
      )?.id == presetB.id
    )
    #expect(
      DisplayPlanner.presetForConnectedSet(
        connected: [displayOne.online, displayTwo.online, displayThree.online],
        presets: [presetA]
      ) == nil
    )
    #expect(
      DisplayPlanner.presetForConnectedSet(
        connected: [displayOne.online],
        presets: [presetA]
      ) == nil
    )
    #expect(
      DisplayPlanner.presetForConnectedSet(
        connected: [displayOne.online],
        presets: [noAutomaticMonitors]
      ) == nil
    )
  }

  @Test(
    "자동 전환 판정은 연결 집합·활성화·정확 일치·우선순위·현재 배치·쿨다운 경계를 지킨다",
    arguments: DisplayAutoSwitchBoundary.allCases
  )
  func automaticSwitchPolicyPreservesSafetyBoundaries(
    _ boundary: DisplayAutoSwitchBoundary
  ) {
    let displayOne = displayFixture(
      id: 101,
      size: DisplaySize(width: 2560, height: 1440)
    )
    let displayTwo = displayFixture(
      id: 102,
      size: DisplaySize(width: 1920, height: 1080)
    )
    let displayThree = displayFixture(
      id: 103,
      size: DisplaySize(width: 1280, height: 720)
    )

    func preset(name: String, automatic: Bool = true) -> DisplayPreset {
      var monitorOne = displayOne.monitor
      monitorOne.autoMatch = automatic
      var monitorTwo = displayTwo.monitor
      monitorTwo.autoMatch = automatic
      return DisplayPreset(
        name: name,
        main: PresetScreen(monitors: [monitorOne, monitorTwo])
      )
    }

    let firstPreset = preset(name: "First")
    let secondPreset = preset(name: "Second")
    var online = [displayOne.online, displayTwo.online]
    var presets = [firstPreset]
    var enabled = true
    var lastJudgedUUIDs: Set<String>? = ["previous-display"]
    var lastAutoApplyAt: TimeInterval?
    let now: TimeInterval = 100
    var expected = DisplayAutoSwitchDecision.ignore

    switch boundary {
    case .unchangedConnectionSet:
      lastJudgedUUIDs = Set(online.map(\.uuid))
    case .disabled:
      enabled = false
    case .noMatchingPreset:
      online = [displayOne.online, displayThree.online]
    case .oneExtraOnlineDisplay:
      online.append(displayThree.online)
    case .oneMissingOnlineDisplay:
      online = [displayOne.online]
    case .noAutomaticMonitors:
      presets = [preset(name: "Manual only", automatic: false)]
    case .firstMatchingPresetWins:
      presets = [firstPreset, secondPreset]
      expected = .apply(index: 0)
    case .currentLayoutAlreadyMatches:
      online[1].currentMirrorMasterID = displayOne.online.id
    case .insideCooldown:
      lastAutoApplyAt = 90
      expected = .retry(after: 20)
    case .cooldownElapsed:
      lastAutoApplyAt = 70
      expected = .apply(index: 0)
    }

    let decision = DisplayAutoSwitchPolicy.decide(
      online: online,
      presets: presets,
      enabled: enabled,
      lastJudgedUUIDs: lastJudgedUUIDs,
      lastAutoApplyAt: lastAutoApplyAt,
      now: now
    )

    #expect(decision == expected, "boundary=\(boundary)")
  }

  @Test("디스플레이 매칭은 유효한 하드웨어 식별을 쓰고 온라인 화면을 재사용하지 않는다")
  func matchingUsesValidHardwareIdentityWithoutReuse() {
    let hardwareFixture = displayFixture(
      id: 41,
      size: DisplaySize(width: 1920, height: 1080)
    )
    var hardwareMatchedOnline = hardwareFixture.online
    hardwareMatchedOnline.uuid = "different-hardware-uuid"
    let hardwareMatches = DisplayMatcher.matches(
      monitors: [hardwareFixture.monitor],
      online: [hardwareMatchedOnline]
    )
    let zeroSerialFixture = displayFixture(
      id: 42,
      size: DisplaySize(width: 1920, height: 1080)
    )
    var zeroSerialMonitor = zeroSerialFixture.monitor
    zeroSerialMonitor.identity.uuid = "preset-zero-serial"
    zeroSerialMonitor.identity.serial = 0
    var zeroSerialOnline = zeroSerialFixture.online
    zeroSerialOnline.uuid = "online-zero-serial"
    zeroSerialOnline.serial = 0
    let zeroSerialMatches = DisplayMatcher.matches(
      monitors: [zeroSerialMonitor],
      online: [zeroSerialOnline]
    )
    let duplicateOnlineMatches = DisplayMatcher.matches(
      monitors: [
        hardwareFixture.monitor,
        PresetMonitor(identity: hardwareFixture.monitor.identity),
      ],
      online: [hardwareFixture.online]
    )

    #expect(hardwareMatches.count == 1)
    #expect(hardwareMatches.first?.onlineIndex == 0)
    #expect(zeroSerialMatches.isEmpty)
    #expect(duplicateOnlineMatches.count == 1)
    #expect(Set(duplicateOnlineMatches.map(\.onlineIndex)).count == duplicateOnlineMatches.count)
  }

  @Test("시드 고정 임의 직선 격자는 겹침·틈이 없고 주 디스플레이 원점이 0이다")
  func seededLinearLayoutsPreserveGeometryInvariants() {
    let seed: UInt64 = 0xED07_0063
    var random = SeededRandom(seed: seed)

    for iteration in 0..<64 {
      let horizontal = Bool.random(using: &random)
      let sizes = (0..<3).map { _ in
        DisplaySize(
          width: Double(Int.random(in: 640...3840, using: &random)),
          height: Double(Int.random(in: 480...2160, using: &random))
        )
      }
      let displays = (0..<3).map { index in
        displayFixture(
          id: UInt32(1_000 + iteration * 3 + index),
          size: sizes[index]
        )
      }
      let negativeCell = horizontal ? GridCell(x: -1, y: 0) : GridCell(x: 0, y: -1)
      let positiveCell = horizontal ? GridCell(x: 1, y: 0) : GridCell(x: 0, y: 1)
      let plan = DisplayPlanner.plan(
        preset: DisplayPreset(
          name: "Seeded layout",
          main: PresetScreen(monitors: [displays[0].monitor]),
          secondary1: PresetScreen(monitors: [displays[1].monitor], cell: negativeCell),
          secondary2: PresetScreen(monitors: [displays[2].monitor], cell: positiveCell)
        ),
        online: displays.map(\.online)
      )
      let message = "seed=\(seed), iteration=\(iteration)"

      #expect(plan.layoutIssues.isEmpty, Comment(rawValue: message))
      #expect(plan.screens.count == 3, Comment(rawValue: message))
      guard plan.screens.count == 3 else { continue }

      let rectangles = plan.screens.map(PlannedRectangle.init)
      let hasOverlap = rectangles.indices.contains { leftIndex in
        rectangles.indices.contains { rightIndex in
          rightIndex > leftIndex && rectangles[leftIndex].overlaps(rectangles[rightIndex])
        }
      }
      let main = plan.screens.first { $0.role == .main }
      let negative = plan.screens.first { $0.cell == negativeCell }
      let positive = plan.screens.first { $0.cell == positiveCell }

      #expect(!hasOverlap, Comment(rawValue: message))
      #expect(main?.origin == .zero, Comment(rawValue: message))
      if horizontal {
        #expect(negative.map { $0.origin.x + $0.size.width } == main?.origin.x, Comment(rawValue: message))
        #expect(positive?.origin.x == main.map { $0.origin.x + $0.size.width }, Comment(rawValue: message))
      } else {
        #expect(negative.map { $0.origin.y + $0.size.height } == main?.origin.y, Comment(rawValue: message))
        #expect(positive?.origin.y == main.map { $0.origin.y + $0.size.height }, Comment(rawValue: message))
      }
    }
  }
}
