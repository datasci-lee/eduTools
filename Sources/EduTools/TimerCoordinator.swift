import Foundation

final class TimerCoordinator {
    private let lessonTimerController: LessonTimerController
    private let breakTimerController: BreakTimerController

    init(defaults: UserDefaults = .standard) {
        lessonTimerController = LessonTimerController(defaults: defaults)
        breakTimerController = BreakTimerController(defaults: defaults)
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
