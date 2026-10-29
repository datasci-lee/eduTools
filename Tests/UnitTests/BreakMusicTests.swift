import Foundation
import Testing

@testable import EduTools

@Suite("휴식 음악")
struct BreakMusicTests {
  @Test("재생목록은 https music.youtube.com 주소만 허용한다")
  func playlistURLRequiresHTTPSMusicHost() {
    #expect(BreakMusicURL.validated("https://music.youtube.com/playlist?list=valid") != nil)
    #expect(BreakMusicURL.validated("https://music.youtube.com.evil.example/playlist") == nil)
    #expect(BreakMusicURL.validated("http://music.youtube.com/playlist") == nil)
  }

  @Test("페이드는 선형이고 종료 시각 변경을 즉시 반영한다")
  func fadeIsLinearAndTracksEndDate() {
    let originalVolume = Float32(0.8)
    let now = Date(timeIntervalSinceReferenceDate: 2_000_000)
    let duringFade = BreakMusicFade.volume(
      originalVolume: originalVolume,
      now: now,
      endDate: now.addingTimeInterval(30),
      duration: 60
    )
    let afterExtension = BreakMusicFade.volume(
      originalVolume: originalVolume,
      now: now,
      endDate: now.addingTimeInterval(120),
      duration: 60
    )

    #expect(abs(duringFade - 0.4) < 0.000_001)
    #expect(abs(afterExtension - originalVolume) < 0.000_001)
  }

  @Test("URL 텍스트는 AppleScript 문자열에 넣기 전에 이스케이프한다")
  func appleScriptLiteralEscapesURLText() {
    #expect(
      AppleScriptString.literal("https://music.youtube.com/a\\\"b\n")
        == "\"https://music.youtube.com/a\\\\\\\"b\\n\""
    )
  }
}
