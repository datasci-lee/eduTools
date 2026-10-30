import AppKit
import Carbon
import Testing
import TestSupport

@testable import EduTools

@Suite("단축키")
struct ShortcutTests {
  @Test("AppMod+P는 파싱·직렬화·표시·물리 키 계약을 지킨다")
  func appModPContract() {
    let shortcut = Shortcut(serialized: "AppMod+P")

    #expect(shortcut != nil)
    #expect(shortcut?.serialized == "AppMod+P")
    #expect(shortcut?.displayString == "⌃⌥P")
    #expect(shortcut?.keyCode == 35)
    #expect(shortcut?.keyCode == UInt32(kVK_ANSI_P))
    #expect(shortcut?.carbonModifiers == Shortcut.appModifierCarbonFlags)
  }

  @Test("AppMod+P는 Carbon 값 왕복 후에도 같은 단축키다")
  func appModPRoundTripsThroughCarbon() {
    let shortcut = Shortcut(serialized: "AppMod+P")
    let reconstructed = shortcut.flatMap {
      Shortcut(keyCode: $0.keyCode, carbonModifiers: $0.carbonModifiers)
    }

    #expect(reconstructed?.serialized == "AppMod+P")
  }

  @Test("AppMod+/는 파싱·직렬화·물리 키 계약을 지킨다")
  func appModSlashContract() {
    let shortcut = Shortcut(serialized: "AppMod+/")

    #expect(shortcut != nil)
    #expect(shortcut?.serialized == "AppMod+/")
    #expect(shortcut?.keyCode == 44)
  }

  @Test("AppMod+/는 Carbon 값 왕복 후에도 같은 단축키다")
  func appModSlashRoundTripsThroughCarbon() {
    let shortcut = Shortcut(serialized: "AppMod+/")
    let reconstructed = shortcut.flatMap {
      Shortcut(keyCode: $0.keyCode, carbonModifiers: $0.carbonModifiers)
    }

    #expect(reconstructed?.serialized == "AppMod+/")
  }

  @Test(
    "잘못된 직렬화 값은 거부된다",
    arguments: [
      "",
      "P",
      "AppMod+",
      "AppMod+p",
      "AppMod+PP",
      "AppMod+P+Z",
      "Control+Option+P",
    ]
  )
  func invalidSerializedValueIsRejected(_ value: String) {
    #expect(Shortcut(serialized: value) == nil)
  }

  @Test("AppMod 없는 Carbon 값은 거부된다")
  func carbonValuesWithoutAppModifierAreRejected() {
    #expect(Shortcut(keyCode: UInt32(kVK_ANSI_P), carbonModifiers: 0) == nil)
  }

  @Test("카탈로그는 같은 범위에 중복 키를 갖지 않는다")
  func catalogBindingsAreUniqueWithinScope() throws {
    let bindingKeys = ShortcutCatalog.definitions.flatMap(\.bindings).map(\.uniquenessKey)
    let catalogFixture = Fixtures.shortcutDefaults(
      bindings: ShortcutCatalog.definitions.flatMap { definition in
        definition.bindings.map { binding in
          ShortcutFixtureBinding(
            definitionID: definition.id,
            scope: binding.scope.rawValue,
            portableString: binding.portableString
          )
        }
      },
      unbound: ShortcutCatalog.definitions.filter(\.bindings.isEmpty).map(\.id)
    )
    let fixture = try Fixtures.decode(
      ShortcutDefaultsFixture.self,
      from: "shortcuts-default.json"
    )

    #expect(Set(bindingKeys).count == bindingKeys.count)
    #expect(catalogFixture == fixture)
  }

  @Test("모든 판서 단축키 명령은 디스패처에 연결된다")
  func annotationCatalogCommandsAreSupported() {
    let annotationDefinitions = ShortcutCatalog.definitions.filter {
      $0.binding(in: .annotation) != nil
    }

    #expect(annotationDefinitions.allSatisfy { AppCommandDispatcher.supports($0.command) })
  }
}

@Suite("키 입력 표시·도움말 검색")
struct ShortcutPresentationTests {
  @Test("수정 키 입력만 표시하고 일반·Shift 단독 타이핑은 거부한다")
  func modifiedKeystrokeDisplayFiltersTyping() {
    let displayedCommandC = KeystrokeDisplayFilter.displayString(
      keyCode: UInt16(kVK_ANSI_C),
      charactersIgnoringModifiers: "c",
      modifierFlags: .command
    )
    let rejectedPlainC = KeystrokeDisplayFilter.displayString(
      keyCode: UInt16(kVK_ANSI_C),
      charactersIgnoringModifiers: "c",
      modifierFlags: []
    )
    let rejectedShiftC = KeystrokeDisplayFilter.displayString(
      keyCode: UInt16(kVK_ANSI_C),
      charactersIgnoringModifiers: "c",
      modifierFlags: .shift
    )

    #expect(displayedCommandC == "⌘C")
    #expect(rejectedPlainC == nil)
    #expect(rejectedShiftC == nil)
  }

  @Test("도움말은 한국어 기능 이름으로 검색된다")
  func helpFilterMatchesKoreanFeatureName() {
    let results = ShortcutCatalog.filtered(ShortcutCatalog.definitions, query: "형광펜")

    #expect(results.map(\.id) == ["tool.highlighter"])
  }

  @Test("도움말은 표시된 단축키로 검색된다")
  func helpFilterMatchesDisplayedKey() {
    let results = ShortcutCatalog.filtered(ShortcutCatalog.definitions, query: "[")

    #expect(results.map(\.id) == ["tool.width.decrease"])
  }
}
