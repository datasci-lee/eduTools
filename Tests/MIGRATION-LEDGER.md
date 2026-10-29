# SelfTest 이관 대장

기준 원본은 `6e873330ddd0d4d89e4eebfa2a2ebd23f05e0d1a`의 `Sources/EduTools/SelfTest.swift`다.

- 원본 통과 수 76 = 정적 `runner.expect` 호출 72 + 76행의 7개 입력 루프 확장 6 - Carbon 파싱 성공 시 실행되지 않는 대체 실패 분기 2(45·63행).
- S1 이관 25호출 = 1~17행과 21~28행. 원본 실행 기준 29확인을 복합 조건 분해 후 Swift Testing `#expect` 34확인으로 옮겼다.
- 이관 뒤 잔여 SelfTest 47확인 + 새 시험 34확인 = 81확인으로 원본 76보다 줄지 않는다.
- 상태 합계: 이관 25, 남음 41(S1b 19, S1c 22), S2 5, S3 1.

| 원본 메시지 | 원본 줄 | 새 시험 ID | 계층 | 상태(이관/S2/S3/남음) |
| --- | ---: | --- | --- | --- |
| AppMod+P should parse | 22 | `UnitTests.ShortcutTests/appModPContract()` | T1 | 이관 |
| AppMod+P should serialize unchanged | 23 | `UnitTests.ShortcutTests/appModPContract()` | T1 | 이관 |
| AppMod+P should use the macOS display form | 24 | `UnitTests.ShortcutTests/appModPContract()` | T1 | 이관 |
| P should use physical keyCode 35 | 25 | `UnitTests.ShortcutTests/appModPContract()` | T1 | 이관 |
| P should use the ANSI physical key constant | 26 | `UnitTests.ShortcutTests/appModPContract()` | T1 | 이관 |
| AppMod should map to Control+Option Carbon flags | 30 | `UnitTests.ShortcutTests/appModPContract()` | T1 | 이관 |
| P should round-trip from Carbon values | 40 | `UnitTests.ShortcutTests/appModPRoundTripsThroughCarbon()` | T1 | 이관 |
| P Carbon round-trip could not run | 45 | `UnitTests.ShortcutTests/appModPRoundTripsThroughCarbon()` | T1 | 이관 |
| AppMod+/ should parse | 49 | `UnitTests.ShortcutTests/appModSlashContract()` | T1 | 이관 |
| AppMod+/ should serialize unchanged | 50 | `UnitTests.ShortcutTests/appModSlashContract()` | T1 | 이관 |
| / should use physical keyCode 44 | 51 | `UnitTests.ShortcutTests/appModSlashContract()` | T1 | 이관 |
| / should round-trip from Carbon values | 58 | `UnitTests.ShortcutTests/appModSlashRoundTripsThroughCarbon()` | T1 | 이관 |
| / Carbon round-trip could not run | 63 | `UnitTests.ShortcutTests/appModSlashRoundTripsThroughCarbon()` | T1 | 이관 |
| `\(value) should be rejected` | 76 | `UnitTests.ShortcutTests/invalidSerializedValueIsRejected(_:)` | T1 | 이관 |
| P without AppMod should be rejected | 78 | `UnitTests.ShortcutTests/carbonValuesWithoutAppModifierAreRejected()` | T1 | 이관 |
| shortcut catalog should not contain duplicate keys in the same scope | 85 | `UnitTests.ShortcutTests/catalogBindingsAreUniqueWithinScope()` | T1 | 이관 |
| every annotation shortcut should be connected to the command dispatcher | 93 | `UnitTests.ShortcutTests/annotationCatalogCommandsAreSupported()` | T1 | 이관 |
| attribute icon width levels should clamp values and preserve every quartile boundary | 98 | `UnitTests.AttrIconTests/widthLevelsFollowVector(_:)` | T1 | 이관 |
| attribute icon conditions should match booleans and values while rejecting unknown attributes | 120 | `UnitTests.AttrIconTests/conditionsFollowVector(_:)` | T1 | 이관 |
| attribute icon contrast should be symmetric and apply halos below the threshold | 152 | `UnitTests.AttrIconTests/contrastFollowsVector(_:)` | T1 | 이관 |
| keystroke display should accept modified shortcuts but reject plain and Shift-only typing | 183 | `UnitTests.ShortcutPresentationTests/modifiedKeystrokeDisplayFiltersTyping()` | T1 | 이관 |
| help filtering should match a Korean feature name | 192 | `UnitTests.ShortcutPresentationTests/helpFilterMatchesKoreanFeatureName()` | T1 | 이관 |
| help filtering should match a displayed shortcut key | 201 | `UnitTests.ShortcutPresentationTests/helpFilterMatchesDisplayedKey()` | T1 | 이관 |
| normalized and screen coordinates should round-trip | 210 | `UnitTests.GeometryTests/normalizedAndScreenCoordinatesRoundTrip()` | T1 | 이관 |
| zoom magnification should move by 0.25 and stay within 1.25 through 8 | 218 | `UnitTests.ZoomTests/magnificationStepsAndClamps()` | T1 | 이관 |
| edge following should stay still in the center and move only past the inner edge | 241 | `UnitTests.ZoomTests/edgeFollowingMovesOnlyBeyondInnerEdge()` | T1 | 이관 |
| zoom center should clamp the viewport to every screen edge | 246 | `UnitTests.ZoomTests/centerClampKeepsViewportOnScreen()` | T1 | 이관 |
| frozen zoom input should map back to the original screen coordinate | 264 | `UnitTests.ZoomTests/frozenInputMapsBackToOriginalCoordinate()` | T1 | 이관 |
| Catmull-Rom smoothing should pass through source points | 284 | — | T1 | 남음(S1b) |
| pen Shift snapping should select 0, 45, and 90 degrees | 299 | — | T1 | 남음(S1b) |
| highlighter Shift snapping should stay horizontal | 305 | — | T1 | 남음(S1b) |
| a nearly straight hand stroke should be recognized | 319 | — | T1 | 남음(S1b) |
| a closed four-sided hand stroke should be recognized as a rectangle | 334 | — | T1 | 남음(S1b) |
| a closed round hand stroke should be recognized as an ellipse | 343 | — | T1 | 남음(S1b) |
| an open zigzag should not be recognized as a shape | 357 | — | T1 | 남음(S1b) |
| lasso containment should respect a concave polygon boundary | 366 | — | T1 | 남음(S1b) |
| selection movement should offset every normalized coordinate | 388 | — | T1 | 남음(S1b) |
| clicking overlapping covers should remove only the topmost matching cover | 421 | — | T1 | 남음(S1b) |
| selection resizing should map normalized coordinates into the new bounds | 432 | — | T1 | 남음(S1b) |
| 101 recorded strokes should permit 100 undos | 460 | — | T1 | 남음(S1b) |
| a new stroke should clear redo history | 486 | — | T1 | 남음(S1b) |
| partial erasing through a stroke center should leave two end fragments | 510 | — | T1 | 남음(S1b) |
| highlighter-only stroke erasing should preserve a touched pen stroke | 542 | — | T1 | 남음(S1b) |
| display ink and its undo history should survive a JSON round-trip | 585 | — | T1 | 남음(S1b) |
| restoring on a different resolution should preserve normalized stroke coordinates | 608 | — | T1 | 남음(S1b) |
| move-to-primary disconnect policy should merge source ink after destination ink | 621 | — | T2 | S2 |
| area erasing should distinguish touching strokes from fully contained strokes | 677 | — | T1 | 남음(S1b) |
| clear all should restore every display with one undo | 692 | — | T2 | S2 |
| screen and board strokes should survive repeated board transitions | 709 | — | T2 | S2 |
| each board page should keep an independent undo history | 728 | — | T2 | S2 |
| a stored black stroke should display as white on the blackboard | 753 | — | T1 | 남음(S1b) |
| lesson remaining time should equal end date minus the injected current time | 761 | `UnitTests.TimerTests/countdownUsesInjectedClock()` | T1 | 이관 |
| a paused lesson timer should stay fixed and continue from that value after resume | 772 | `UnitTests.TimerTests/pauseFreezesAndResumeContinues()` | T1 | 이관 |
| the +10 minute break button should calculate its end date from the current clock | 777 | `UnitTests.TimerTests/tenMinuteBreakUsesInjectedClock()` | T1 | 이관 |
| a direct break time earlier than now should be rejected as already passed today | 788 | `UnitTests.TimerTests/pastDirectBreakTimeIsRejected()` | T1 | 이관 |
| lesson badge snapping should distinguish positions inside and outside 16 pixels | 812 | `UnitTests.TimerTests/badgeSnapsInsideSixteenPixelsOnly()` | T1 | 이관 |
| break music should accept only https://music.youtube.com/ playlist URLs | 822 | `UnitTests.BreakMusicTests/playlistURLRequiresHTTPSMusicHost()` | T1 | 이관 |
| break music fade should be linear and move when the break end date changes | 841 | `UnitTests.BreakMusicTests/fadeIsLinearAndTracksEndDate()` | T1 | 이관 |
| break music should escape URL text before inserting it into AppleScript | 847 | `UnitTests.BreakMusicTests/appleScriptLiteralEscapesURLText()` | T1 | 이관 |
| display grid planning should place all tested layouts edge-to-edge from the origin | 957 | `UnitTests.DisplayPlannerTests/gridLayoutsFollowVector(_:)` | T1 | 이관 |
| display planning should anchor the connected main display at zero and reject overlaps | 1028 | `UnitTests.DisplayPlannerTests/connectedMainAnchorsAtZeroAndOverlapIsRejected()` | T1 | 이관 |
| display planning should preserve mirror, master succession, missing, and outside monitor rules | 1086 | `UnitTests.DisplayPlannerTests/mirrorSuccessionMissingAndOutsideRulesArePreserved()` | T1 | 이관 |
| display grid validation should reject duplicate or diagonal cells and accept connected cells | 1130 | `UnitTests.DisplayPlannerTests/gridValidationRejectsDuplicateAndDiagonalCells()` | T1 | 이관 |
| automatic display preset selection should require an exact set and preserve list priority | 1153 | `UnitTests.DisplayPlannerTests/automaticSelectionRequiresExactSetAndPreservesPriority()` | T1 | 이관 |
| display matching should use valid hardware identity without reusing an online display | 1208 | `UnitTests.DisplayPlannerTests/matchingUsesValidHardwareIdentityWithoutReuse()` | T1 | 이관 |
| settings JSON should round-trip without changing values | 1222 | `UnitTests.SettingsTests/jsonRoundTripPreservesValues()` | T1 | 이관 |
| settings validation should reject a zoom value outside 1.25 through 8 | 1236 | `UnitTests.SettingsTests/validationRejectsOutOfRangeZoom()` | T1 | 이관 |
| settings decoding should ignore unknown keys and default missing compatible values | 1266 | `UnitTests.SettingsTests/decodingIgnoresUnknownKeysAndDefaultsMissingValues()` | T1 | 이관 |
| settings export should include display presets but exclude computer-specific switches and paths | 1310 | `UnitTests.SettingsTests/exportIncludesPresetsAndExcludesComputerSpecificValues()` | T1 | 이관 |
| known avoided global shortcuts should be reported as conflicts | 1321 | `UnitTests.SettingsTests/avoidedGlobalShortcutIsReportedAsConflict()` | T1 | 이관 |
| settings backups should rotate to the most recent five entries | 1334 | — | T2 | S2 |
| synthetic enter and exit events should guard device queries and the event-kind table should remain complete | 1362 | — | T3 | S3 |
