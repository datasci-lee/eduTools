import Foundation

final class TimerCoordinator {
    private let lessonTimerController: LessonTimerController
    private let breakTimerController: BreakTimerController

    init(defaults: UserDefaults = .standard, settingsStore: SettingsStore? = nil) {
        let resolvedSettingsStore = settingsStore ?? SettingsStore(defaults: defaults)
        lessonTimerController = LessonTimerController(
            defaults: defaults,
            settingsStore: resolvedSettingsStore
        )
        breakTimerController = BreakTimerController(
            defaults: defaults,
            settingsStore: resolvedSettingsStore
        )
        lessonTimerController.onAutoBreakRequested = { [weak breakTimerController] minutes in
            breakTimerController?.startQuick(minutes: minutes)
        }
    }

    func toggleLessonTimer() {
        lessonTimerController.toggleFromShortcut()
    }

    func openBreakTimerSetup() {
        breakTimerController.openSetup()
    }

    func setLessonDisplayHandler(_ handler: @escaping (String) -> Void) {
        lessonTimerController.setDisplayTextHandler(handler)
    }

    func stopBreakMusicForTermination() {
        breakTimerController.stopMusicForTermination()
    }
}
