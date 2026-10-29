import Testing

@testable import EduTools

@Suite("파일명")
struct FileNameTests {
  @Test(
    "표시 UUID는 기존 규칙대로 안전한 파일명 구성 요소가 된다",
    arguments: [
      ("A-b_1", "A-b_1"),
      ("/", "_"),
      (":", "_"),
      (" ", "_"),
      (".", "_"),
      ("..", "__"),
      ("a/b", "a_b"),
      ("../x", "___x"),
      ("한글English123", "한글English123"),
      ("", ""),
    ]
  )
  func displayUUIDFileComponentPreservesExistingRule(_ input: String, _ expected: String) {
    #expect(fileComponent(forDisplayUUID: input) == expected)
  }
}
