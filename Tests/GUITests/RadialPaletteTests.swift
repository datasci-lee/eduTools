import AppKit
import Carbon
import Testing

@testable import EduTools
import TestSupport

@MainActor
@Suite("원형 팔레트 GUI", .serialized)
struct RadialPaletteTests {
  @Test("즐겨찾기 최대 9칸은 겹치지 않고 각 칸 중심에서 히트된다")
  func maximumFavoriteCellsDoNotOverlapAndHitTest() throws {
    requireWindowServer()
    _ = NSApplication.shared
    let fixture = try RadialPaletteFixture(favoriteCount: 8)
    fixture.showAndExpand()

    guard
      let favoritesGroup = fixture.controller.layoutCells().first(where: { cell in
        guard case let .group(group) = cell.action else { return false }
        return group == .favorites
      })
    else {
      Issue.record("즐겨찾기 그룹 칸을 찾지 못했습니다")
      return
    }
    fixture.controller.activate(.cell(favoritesGroup))
    let entries = fixture.controller.layoutCells().filter { $0.ring == .entry }

    #expect(entries.count == 9)
    #expect(
      zip(entries, entries.dropFirst()).allSatisfy { previous, next in
        previous.endAngle <= next.startAngle
      }
    )

    guard let targetCell = entries.last else {
      Issue.record("히트 테스트할 즐겨찾기 칸이 없습니다")
      return
    }
    let angle = (targetCell.startAngle + targetCell.endAngle) / 2
    let radius = (targetCell.innerRadius + targetCell.outerRadius) / 2
    let bounds = fixture.controller.window.contentView?.bounds
      ?? CGRect(origin: .zero, size: fixture.controller.window.frame.size)
    let point = CGPoint(
      x: bounds.midX + sin(angle) * radius,
      y: bounds.midY + cos(angle) * radius
    )
    let hit = fixture.controller.hitTarget(at: point, in: bounds)

    #expect(hit?.identifier == targetCell.identifier)
  }

  @Test("방향키로 다음 도구 칸으로 이동한 뒤 Return이 그 도구를 실행한다")
  func arrowThenReturnMovesAndExecutesFocusedEntry() throws {
    requireWindowServer()
    _ = NSApplication.shared
    let fixture = try RadialPaletteFixture()
    fixture.showAndExpand()

    try fixture.sendKey(keyCode: UInt16(kVK_RightArrow), characters: "\u{F703}")
    try fixture.sendKey(keyCode: UInt16(kVK_Return), characters: "\r")

    #expect(fixture.state.currentTool == .highlighter)
  }

  @Test("지우개 방식 칸은 도구와 방식을 함께 고르고 영역 칸 재선택은 모양을 전환한다")
  func eraserModeEntriesSelectOneModeAndRetoggleAreaShape() throws {
    requireWindowServer()
    _ = NSApplication.shared
    let fixture = try RadialPaletteFixture()
    fixture.showAndExpand()

    guard let erasingGroup = fixture.controller.layoutCells().first(where: { cell in
      guard case let .group(group) = cell.action else { return false }
      return group == .erasing
    }) else {
      Issue.record("지우기 그룹 칸을 찾지 못했습니다")
      return
    }
    fixture.controller.activate(.cell(erasingGroup))

    let eraserEntries = fixture.controller.layoutCells().filter { cell in
      guard case let .entry(entry) = cell.action else { return false }
      return entry.eraserMode != nil
    }
    #expect(eraserEntries.count == 3)
    #expect(Set(eraserEntries.map(\.identifier)).count == 3)

    guard let areaEntry = eraserEntries.first(where: { cell in
      guard case let .entry(entry) = cell.action else { return false }
      return entry.eraserMode == .area
    }) else {
      Issue.record("영역 지우개 칸을 찾지 못했습니다")
      return
    }
    fixture.controller.activate(.cell(areaEntry))

    #expect(fixture.state.currentTool == .eraser)
    #expect(fixture.state.eraserMode == .area)
    let selectedEraserEntries = fixture.controller.layoutCells().filter { cell in
      guard case let .entry(entry) = cell.action, entry.eraserMode != nil else { return false }
      return cell.isSelected
    }
    #expect(selectedEraserEntries.map(\.identifier) == [areaEntry.identifier])

    let originalAreaShape = fixture.state.eraserAreaShape
    fixture.controller.activate(.cell(areaEntry))
    #expect(fixture.state.eraserAreaShape != originalAreaShape)
  }

  @Test("옵션 칸은 30도 이하이며 단일 섹션은 그룹 반대편, 두 섹션은 12시부터 배치된다")
  func optionRingCapsSlicesAndAlignsSections() throws {
    requireWindowServer()
    _ = NSApplication.shared
    let fixture = try RadialPaletteFixture()
    fixture.showAndExpand()

    let fullCircle = 2 * CGFloat.pi
    let groupSlice = fullCircle / CGFloat(RadialPaletteGroupID.allCases.count)
    let tolerance: CGFloat = 0.0001

    func normalized(_ angle: CGFloat) -> CGFloat {
      let remainder = angle.truncatingRemainder(dividingBy: fullCircle)
      return remainder >= 0 ? remainder : remainder + fullCircle
    }

    func angularDistance(_ lhs: CGFloat, _ rhs: CGFloat) -> CGFloat {
      let difference = abs(normalized(lhs) - normalized(rhs))
      return min(difference, fullCircle - difference)
    }

    func groupCell(_ group: RadialPaletteGroupID) throws -> RadialPaletteCell {
      try #require(fixture.controller.layoutCells().first { cell in
        guard case let .group(candidate) = cell.action else { return false }
        return candidate == group
      })
    }

    func entryCell(for tool: InkTool) throws -> RadialPaletteCell {
      try #require(fixture.controller.layoutCells().first { cell in
        guard case let .entry(entry) = cell.action else { return false }
        return entry.inkTool == tool
      })
    }

    fixture.controller.activate(.cell(try groupCell(.presenting)))
    fixture.controller.activate(.cell(try entryCell(for: .laser)))
    let laserOptions = fixture.controller.layoutCells().filter { $0.ring == .option }
    let laserFirst = try #require(laserOptions.first)
    let laserLast = try #require(laserOptions.last)
    let laserArcStart = laserFirst.startAngle - RadialPaletteMetrics.sectorGap / 2
    let laserArcEnd = laserLast.endAngle + RadialPaletteMetrics.sectorGap / 2
    let laserArcCenter = (laserArcStart + laserArcEnd) / 2
    let presentingOpposite = CGFloat(RadialPaletteGroupID.presenting.rawValue) * groupSlice + .pi

    #expect(laserOptions.count == 6)
    #expect(laserOptions.allSatisfy { cell in
      cell.endAngle - cell.startAngle <= RadialPaletteMetrics.optionMaximumSlice + tolerance
    })
    #expect(laserFirst.startAngle >= 0)
    #expect(laserLast.endAngle > fullCircle)
    #expect(angularDistance(laserArcCenter, presentingOpposite) < tolerance)

    fixture.controller.activate(.cell(try groupCell(.writing)))
    fixture.controller.activate(.cell(try entryCell(for: .pen)))
    let penOptions = fixture.controller.layoutCells().filter { $0.ring == .option }
    let penFirst = try #require(penOptions.first)

    #expect(penOptions.count == 11)
    guard penOptions.count > 6 else { return }
    #expect(abs(
      penFirst.startAngle - RadialPaletteMetrics.sectorGap / 2
        - RadialPaletteMetrics.propertyGap / 2
    ) < tolerance)
    #expect(
      penOptions[6].startAngle - penOptions[5].endAngle
        >= RadialPaletteMetrics.propertyGap
    )
  }
}

@MainActor
private final class RadialPaletteFixture {
  let state: AppState
  let controller: RadialPaletteController

  private let defaults: UserDefaults
  private let suiteName: String

  init(favoriteCount: Int = 0) throws {
    suiteName = "EduTools.GUITests.RadialPalette.\(UUID().uuidString)"
    defaults = UserDefaults(suiteName: suiteName)!
    let settingsStore = SettingsStore(defaults: defaults)
    if favoriteCount > 0 {
      var settings = settingsStore.settings
      settings.tools.favorites = (0..<favoriteCount).map { index in
        FavoriteTool(
          tool: InkTool.pen.rawValue,
          colorIndex: 0,
          size: Double(index + 1),
          arrowStyle: nil,
          eraserMode: nil,
          eraserAreaShape: nil
        )
      }
      try settingsStore.apply(settings)
    }
    state = AppState(defaults: defaults, settingsStore: settingsStore)
    let dispatcher = AppCommandDispatcher(state: state)
    controller = RadialPaletteController(
      state: state,
      dispatcher: dispatcher,
      settingsStore: settingsStore,
      defaults: defaults,
      openSettings: {}
    )
    dispatcher.radialPaletteController = controller
    controller.window.alphaValue = 0
  }

  deinit {
    defaults.removePersistentDomain(forName: suiteName)
  }

  func showAndExpand() {
    state.setAnnotationModeEnabled(true)
    controller.toggleExpansionFromKeyboard()
  }

  func sendKey(keyCode: UInt16, characters: String) throws {
    let down = try SyntheticEvents.keyEvent(
      type: .keyDown,
      keyCode: keyCode,
      characters: characters,
      in: controller.window,
      timestamp: 1
    )
    let up = try SyntheticEvents.keyEvent(
      type: .keyUp,
      keyCode: keyCode,
      characters: characters,
      in: controller.window,
      timestamp: 2
    )
    #expect(controller.consumePaletteKeyboardEvent(down))
    #expect(controller.consumePaletteKeyboardEvent(up))
  }
}
