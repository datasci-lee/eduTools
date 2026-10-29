import AppKit
import Testing

@testable import EduTools

enum RadialNavigationScenario: Sendable {
  case groupWithCurrentTool
  case groupWithoutCurrentTool
  case groupWithSelectedIdentifier
  case entryToOption
  case optionToEntry
}

struct RadialNavigationExpectation: Sendable {
  let scenario: RadialNavigationScenario
  let expectedIdentifier: String
}

@Suite("원형 팔레트 키보드 이동")
struct RadialPaletteNavigationTests {
  @Test(
    "현재 포커스와 방향 입력은 원본 이동 표의 다음 포커스를 고른다",
    arguments: [
      RadialNavigationExpectation(
        scenario: .groupWithCurrentTool,
        expectedIdentifier: "entry.tool.highlighter"
      ),
      RadialNavigationExpectation(
        scenario: .groupWithoutCurrentTool,
        expectedIdentifier: "entry.tool.pen"
      ),
      RadialNavigationExpectation(
        scenario: .groupWithSelectedIdentifier,
        expectedIdentifier: "entry.palette.display-presets"
      ),
      RadialNavigationExpectation(
        scenario: .entryToOption,
        expectedIdentifier: "option.pen.color.1"
      ),
      RadialNavigationExpectation(
        scenario: .optionToEntry,
        expectedIdentifier: "entry.tool.pen"
      ),
    ]
  )
  func navigationMatchesOriginalTable(_ expectation: RadialNavigationExpectation) {
    let input = navigationInput(for: expectation.scenario)

    let result = nextRadialPaletteFocus(
      in: input.cells,
      from: input.current,
      for: input.key,
      selectedEntryIdentifier: input.selectedEntryIdentifier
    )

    #expect(result.identifier == expectation.expectedIdentifier)
  }

  private func navigationInput(
    for scenario: RadialNavigationScenario
  ) -> (
    cells: [RadialPaletteCell],
    current: RadialPaletteFocus,
    key: RadialPaletteNavigationKey,
    selectedEntryIdentifier: String?
  ) {
    let group = groupCell()
    let pen = entryCell(.command(.selectTool(.pen)), isSelected: false)
    let selectedPen = entryCell(.command(.selectTool(.pen)), isSelected: true)
    let highlighter = entryCell(.command(.selectTool(.highlighter)), isSelected: false)
    let selectedHighlighter = entryCell(
      .command(.selectTool(.highlighter)),
      isSelected: true
    )
    let laser = entryCell(.command(.useLaser), isSelected: true)
    let displayPresets = entryCell(.displayPresets, isSelected: false)
    let option = optionCell(identifier: "pen.color.1")

    switch scenario {
    case .groupWithCurrentTool:
      return ([group, pen, selectedHighlighter], .cell(group.identifier), .deeper, nil)
    case .groupWithoutCurrentTool:
      return ([group, pen, highlighter], .cell(group.identifier), .deeper, nil)
    case .groupWithSelectedIdentifier:
      return (
        [group, laser, displayPresets],
        .cell(group.identifier),
        .deeper,
        displayPresetsEntryIdentifier
      )
    case .entryToOption:
      return (
        [group, selectedPen, option],
        .cell(selectedPen.identifier),
        .deeper,
        penEntryIdentifier
      )
    case .optionToEntry:
      return (
        [group, selectedPen, option],
        .cell(option.identifier),
        .shallower,
        penEntryIdentifier
      )
    }
  }

  private var penEntryIdentifier: String { "tool.pen" }
  private var displayPresetsEntryIdentifier: String { "palette.display-presets" }

  private func groupCell() -> RadialPaletteCell {
    RadialPaletteCell(
      identifier: "group.0",
      ring: .group,
      geometry: geometry,
      name: "작성",
      iconKey: nil,
      iconName: nil,
      shortLabel: "작성",
      shortcut: "",
      propertyName: nil,
      color: nil,
      isSelected: true,
      isEnabled: true,
      action: .group(.writing)
    )
  }

  private func entryCell(
    _ entry: RadialPaletteEntry,
    isSelected: Bool
  ) -> RadialPaletteCell {
    RadialPaletteCell(
      identifier: "entry.\(entry.identifier)",
      ring: .entry,
      geometry: geometry,
      name: entry.name,
      iconKey: entry.iconKey,
      iconName: entry.iconName,
      shortLabel: "",
      shortcut: entry.shortcut,
      propertyName: nil,
      color: nil,
      isSelected: isSelected,
      isEnabled: true,
      action: .entry(entry)
    )
  }

  private func optionCell(identifier: String) -> RadialPaletteCell {
    let option = RadialPaletteOption(
      identifier: identifier,
      name: "색 1",
      shortLabel: "1",
      iconKey: nil,
      iconName: nil,
      color: nil,
      shortcut: "1",
      isSelected: true,
      action: {}
    )
    return RadialPaletteCell(
      identifier: "option.\(identifier)",
      ring: .option,
      geometry: geometry,
      name: option.name,
      iconKey: nil,
      iconName: nil,
      shortLabel: option.shortLabel,
      shortcut: option.shortcut,
      propertyName: "색",
      color: nil,
      isSelected: true,
      isEnabled: true,
      action: .option(option)
    )
  }

  private var geometry: RadialPaletteSectorGeometry {
    RadialPaletteSectorGeometry(innerRadius: 0, outerRadius: 1, startAngle: 0, endAngle: 1)
  }
}
