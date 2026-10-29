import AppKit
import Testing
import TestSupport

@testable import EduTools

@Suite("AppState 합성")
struct AppStateIntegrationTests {
  @Test("연결 해제 화면 판서는 주 화면의 기존 판서 뒤에 병합된다")
  func moveToPrimaryMergesAfterDestinationInk() throws {
    let defaults = try IsolatedDefaults()
    let tempDir = try TempDir()
    let state = IntegrationFixtures.state(defaults: defaults.value)
    let destination = IntegrationFixtures.stroke(
      tool: .pen,
      color: .systemBlue,
      location: CGPoint(x: 0.2, y: 0.2),
      width: 3
    )
    let sourceFirst = IntegrationFixtures.stroke(
      tool: .pen,
      color: .systemRed,
      location: CGPoint(x: 0.4, y: 0.4),
      width: 4
    )
    let sourceSecond = IntegrationFixtures.stroke(
      tool: .highlighter,
      color: .systemYellow,
      location: CGPoint(x: 0.6, y: 0.6),
      width: 18
    )
    state.addStroke(destination, to: "primary-display")
    state.addStroke(sourceFirst, to: "removed-display")
    state.addStroke(sourceSecond, to: "removed-display")

    let moved = state.moveScreenInk(
      from: "removed-display",
      to: "primary-display"
    )

    #expect(moved)
    #expect(state.screenStrokes(for: "removed-display").isEmpty)
    #expect(
      state.screenStrokes(for: "primary-display").map(\.identifier)
        == [destination.identifier, sourceFirst.identifier, sourceSecond.identifier]
    )
    withExtendedLifetime(tempDir) {}
  }

  @Test("전체 지우기는 여러 화면을 한 번의 undo로 함께 복원한다")
  func clearAllRestoresEveryDisplayWithOneUndo() throws {
    let defaults = try IsolatedDefaults()
    _ = try TempDir()
    let state = IntegrationFixtures.state(defaults: defaults.value)
    let first = IntegrationFixtures.stroke(
      tool: .pen,
      color: .systemBlue,
      location: CGPoint(x: 0.3, y: 0.3),
      width: 3
    )
    let second = IntegrationFixtures.stroke(
      tool: .highlighter,
      color: .systemYellow,
      location: CGPoint(x: 0.7, y: 0.7),
      width: 18
    )
    state.addStroke(first, to: "display-a")
    state.addStroke(second, to: "display-b")

    #expect(state.clearAllInk())
    #expect(state.strokes(for: "display-a").isEmpty)
    #expect(state.strokes(for: "display-b").isEmpty)
    #expect(state.undo(on: "display-a"))
    #expect(state.strokes(for: "display-a") == [first])
    #expect(state.strokes(for: "display-b") == [second])
  }

  @Test("화면과 보드를 반복 전환해도 각 판서가 따로 유지된다")
  func repeatedBoardTransitionsPreserveScreenAndBoardInk() throws {
    let defaults = try IsolatedDefaults()
    _ = try TempDir()
    let state = IntegrationFixtures.state(defaults: defaults.value)
    let screenStroke = IntegrationFixtures.stroke(
      tool: .pen,
      color: .systemBlue,
      location: CGPoint(x: 0.25, y: 0.25),
      width: 3
    )
    let boardStroke = IntegrationFixtures.stroke(
      tool: .highlighter,
      color: .systemYellow,
      location: CGPoint(x: 0.75, y: 0.75),
      width: 18
    )
    state.addStroke(screenStroke, to: "transition-display")

    state.toggleBoard(.white, on: "transition-display")
    state.addStroke(boardStroke, to: "transition-display")
    #expect(state.strokes(for: "transition-display") == [boardStroke])

    state.toggleBoard(.white, on: "transition-display")
    #expect(state.strokes(for: "transition-display") == [screenStroke])

    state.toggleBoard(.white, on: "transition-display")
    #expect(state.strokes(for: "transition-display") == [boardStroke])
  }

  @Test("보드 페이지마다 undo 기록이 독립적으로 유지된다")
  func boardPagesKeepIndependentUndoHistories() throws {
    let defaults = try IsolatedDefaults()
    _ = try TempDir()
    let state = IntegrationFixtures.state(defaults: defaults.value)
    let firstPageStroke = IntegrationFixtures.stroke(
      tool: .pen,
      color: .systemBlue,
      location: CGPoint(x: 0.25, y: 0.25),
      width: 3
    )
    let secondPageStroke = IntegrationFixtures.stroke(
      tool: .highlighter,
      color: .systemYellow,
      location: CGPoint(x: 0.75, y: 0.75),
      width: 18
    )
    state.toggleBoard(.white, on: "page-display")
    state.addStroke(firstPageStroke, to: "page-display")

    #expect(state.moveToNextBoardPage(on: "page-display"))
    state.addStroke(secondPageStroke, to: "page-display")
    #expect(state.undo(on: "page-display"))
    #expect(state.strokes(for: "page-display").isEmpty)

    #expect(state.moveToPreviousBoardPage(on: "page-display"))
    #expect(state.strokes(for: "page-display") == [firstPageStroke])
    #expect(state.undo(on: "page-display"))
    #expect(state.strokes(for: "page-display").isEmpty)

    #expect(state.moveToNextBoardPage(on: "page-display"))
    #expect(state.strokes(for: "page-display").isEmpty)
    #expect(
      state.boardPagePosition(on: "page-display")
        == BoardPagePosition(current: 2, total: 2)
    )
  }
}

@Suite("설정 백업")
struct SettingsBackupIntegrationTests {
  @Test("여섯 번째 백업은 가장 오래된 항목을 버리고 최근 다섯 개만 남긴다")
  func backupsRotateToMostRecentFive() throws {
    let defaults = try IsolatedDefaults()
    _ = try TempDir()
    let backupStore = SettingsBackupStore(defaults: defaults.value)

    for minutes in 1...6 {
      var settings = AppSettings.default
      settings.timer.lessonDefaultMinutes = minutes
      try backupStore.append(settings)
    }

    #expect(backupStore.count == 5)
    let latest = try backupStore.popLatest()
    #expect(latest?.timer.lessonDefaultMinutes == 6)
    #expect(backupStore.count == 4)
  }
}
