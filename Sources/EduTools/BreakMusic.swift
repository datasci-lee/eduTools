import AppKit
import AVFoundation
import CoreAudio
import Foundation

enum BreakMusicBrowser: String {
    case chrome
    case youtubeMusicApp
    case defaultBrowser
}

enum BreakMusicURL {
    static let requiredPrefix = "https://music.youtube.com/"

    static func validated(_ value: String) -> URL? {
        guard value.hasPrefix(requiredPrefix),
              !value.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }),
              let components = URLComponents(string: value),
              components.scheme == "https",
              components.host == "music.youtube.com",
              components.user == nil,
              components.password == nil,
              let url = components.url
        else {
            return nil
        }
        return url
    }
}

enum BreakMusicFade {
    static func volume(
        originalVolume: Float32,
        now: Date,
        endDate: Date,
        duration: TimeInterval
    ) -> Float32 {
        guard duration > 0 else { return originalVolume }
        let remaining = endDate.timeIntervalSince(now)
        guard remaining < duration else { return originalVolume }
        guard remaining > 0 else { return 0 }
        return originalVolume * Float32(remaining / duration)
    }
}

enum AppleScriptString {
    static func literal(_ value: String) -> String {
        var escaped = ""
        escaped.reserveCapacity(value.count + 2)
        for character in value {
            switch character {
            case "\\": escaped += "\\\\"
            case "\"": escaped += "\\\""
            case "\r": escaped += "\\r"
            case "\n": escaped += "\\n"
            case "\t": escaped += "\\t"
            default: escaped.append(character)
            }
        }
        return "\"\(escaped)\""
    }
}

final class BreakMusicSettings {
    static let maximumPlaylistCount = 5
    static var defaultFadeDuration: TimeInterval { AppSettings.default.timer.fadeDuration }

    private let settingsStore: SettingsStore

    var playlistURLs: [URL] {
        settingsStore.settings.timer.playlistURLs.compactMap(BreakMusicURL.validated)
    }
    var defaultPlaylistIndex: Int { settingsStore.settings.timer.defaultPlaylistIndex }
    var browser: BreakMusicBrowser {
        BreakMusicBrowser(rawValue: settingsStore.settings.timer.musicBrowser) ?? .chrome
    }
    var isFadeEnabled: Bool { settingsStore.settings.timer.fadeEnabled }
    var fadeDuration: TimeInterval { settingsStore.settings.timer.fadeDuration }
    var isMusicEnabled: Bool { settingsStore.settings.timer.musicEnabled }
    var source: BreakMusicSource {
        BreakMusicSource(rawValue: settingsStore.settings.timer.musicSource) ?? .youtube
    }
    var musicFolderURL: URL? {
        let path = settingsStore.settings.timer.musicFolderPath
        guard !path.isEmpty else { return nil }
        return URL(fileURLWithPath: path, isDirectory: true)
    }
    var order: BreakMusicOrder {
        BreakMusicOrder(rawValue: settingsStore.settings.timer.musicOrder) ?? .name
    }
    var repeatMode: BreakMusicRepeatMode {
        BreakMusicRepeatMode(rawValue: settingsStore.settings.timer.musicRepeat) ?? .all
    }

    init(settingsStore: SettingsStore) {
        self.settingsStore = settingsStore
    }

    convenience init(defaults: UserDefaults = .standard) {
        self.init(settingsStore: SettingsStore(defaults: defaults))
    }

    var selectedPlaylistURL: URL? {
        guard playlistURLs.indices.contains(defaultPlaylistIndex) else { return nil }
        return playlistURLs[defaultPlaylistIndex]
    }

    @discardableResult
    func replacePlaylistURLs(with values: [String], defaultIndex: Int) -> Bool {
        guard values.count <= Self.maximumPlaylistCount else { return false }
        let validatedURLs = values.compactMap(BreakMusicURL.validated)
        guard validatedURLs.count == values.count,
              validatedURLs.isEmpty || validatedURLs.indices.contains(defaultIndex)
        else {
            return false
        }

        settingsStore.update {
            $0.timer.playlistURLs = validatedURLs.map(\.absoluteString)
            $0.timer.defaultPlaylistIndex = validatedURLs.isEmpty ? 0 : defaultIndex
        }
        return true
    }

    @discardableResult
    func selectPlaylist(at index: Int) -> Bool {
        guard playlistURLs.indices.contains(index) else { return false }
        settingsStore.update { $0.timer.defaultPlaylistIndex = index }
        return true
    }

    func setMusicEnabled(_ isEnabled: Bool) {
        settingsStore.update { $0.timer.musicEnabled = isEnabled }
    }

    func setBrowser(_ browser: BreakMusicBrowser) {
        settingsStore.update { $0.timer.musicBrowser = browser.rawValue }
    }

    func setFade(enabled: Bool, duration: TimeInterval) {
        guard duration > 0 else {
            NSLog("EduTools: rejected a non-positive break music fade duration")
            return
        }
        settingsStore.update {
            $0.timer.fadeEnabled = enabled
            $0.timer.fadeDuration = duration
        }
    }
}

private struct SystemVolumeSnapshot {
    let deviceID: AudioObjectID
    let volume: Float32
}

private final class CoreAudioVolumeController {
    func captureDefaultOutputVolume() -> SystemVolumeSnapshot? {
        var defaultDevice = AudioObjectID(kAudioObjectUnknown)
        var deviceSize = UInt32(MemoryLayout<AudioObjectID>.size)
        var defaultDeviceAddress = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        let deviceStatus = AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject),
            &defaultDeviceAddress,
            0,
            nil,
            &deviceSize,
            &defaultDevice
        )
        guard deviceStatus == noErr, defaultDevice != kAudioObjectUnknown else { return nil }

        var volumeAddress = Self.volumeAddress
        guard AudioObjectHasProperty(defaultDevice, &volumeAddress) else { return nil }
        var isSettable = DarwinBoolean(false)
        guard AudioObjectIsPropertySettable(defaultDevice, &volumeAddress, &isSettable) == noErr,
              isSettable.boolValue
        else {
            return nil
        }

        var volume = Float32(0)
        var volumeSize = UInt32(MemoryLayout<Float32>.size)
        guard AudioObjectGetPropertyData(
            defaultDevice,
            &volumeAddress,
            0,
            nil,
            &volumeSize,
            &volume
        ) == noErr else {
            return nil
        }
        return SystemVolumeSnapshot(deviceID: defaultDevice, volume: volume)
    }

    func setVolume(_ volume: Float32, on snapshot: SystemVolumeSnapshot) -> Bool {
        var clampedVolume = min(max(volume, 0), 1)
        var volumeAddress = Self.volumeAddress
        return AudioObjectSetPropertyData(
            snapshot.deviceID,
            &volumeAddress,
            0,
            nil,
            UInt32(MemoryLayout<Float32>.size),
            &clampedVolume
        ) == noErr
    }

    private static var volumeAddress: AudioObjectPropertyAddress {
        AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyVolumeScalar,
            mScope: kAudioDevicePropertyScopeOutput,
            mElement: kAudioObjectPropertyElementMain
        )
    }
}

private enum PlaybackLaunchState {
    case opening
    case opened
}

private struct ChromeTabReference {
    let windowID: Int32
    let tabID: Int32
    let url: URL
}

private struct ActiveBreakMusicPlayback {
    let token: UUID
    let url: URL
    var usesChrome: Bool
    var launchState: PlaybackLaunchState
    var chromeTabReference: ChromeTabReference?
}

final class BreakMusicController: NSObject, AVAudioPlayerDelegate {
    let settings: BreakMusicSettings
    var bringBreakScreenToFront: (() -> Void)?
    var showMessage: ((String) -> Void)?

    private let workspace: NSWorkspace
    private let playlistCollector: MusicPlaylistCollector
    private let volumeController = CoreAudioVolumeController()
    private var volumeSnapshot: SystemVolumeSnapshot?
    private var activePlayback: ActiveBreakMusicPlayback?
    private var playbackSource: BreakMusicSource?
    private var folderScanToken: UUID?
    private var folderPlaylist: MusicPlaylist?
    private var audioPlayer: AVAudioPlayer?
    private var folderVolume: Float32 = 1
    private var isBreakActive = false
    private var isFadeAvailable = true

    init(
        settings: BreakMusicSettings,
        workspace: NSWorkspace = .shared,
        playlistCollector: MusicPlaylistCollector = MusicPlaylistCollector()
    ) {
        self.settings = settings
        self.workspace = workspace
        self.playlistCollector = playlistCollector
        super.init()
    }

    func breakDidStart() {
        isBreakActive = true
        guard settings.isMusicEnabled else { return }
        startPlayback()
    }

    func breakDidUpdate(now: Date, endDate: Date) {
        guard isBreakActive,
              settings.isMusicEnabled,
              settings.isFadeEnabled
        else {
            return
        }
        if playbackSource == .folder {
            folderVolume = BreakMusicFade.volume(
                originalVolume: 1,
                now: now,
                endDate: endDate,
                duration: settings.fadeDuration
            )
            audioPlayer?.volume = folderVolume
            return
        }
        guard isFadeAvailable, let volumeSnapshot else { return }
        let targetVolume = BreakMusicFade.volume(
            originalVolume: volumeSnapshot.volume,
            now: now,
            endDate: endDate,
            duration: settings.fadeDuration
        )
        guard volumeController.setVolume(targetVolume, on: volumeSnapshot) else {
            isFadeAvailable = false
            NSLog("EduTools: break music fade stopped because the output volume could not be changed")
            return
        }
    }

    func breakDidEnd() {
        guard isBreakActive
                || activePlayback != nil
                || volumeSnapshot != nil
                || folderScanToken != nil
                || folderPlaylist != nil
                || audioPlayer != nil
        else {
            return
        }
        isBreakActive = false
        stopPlayback()
        restoreVolume()
    }

    func toggleMusic() {
        let shouldEnable = !settings.isMusicEnabled
        settings.setMusicEnabled(shouldEnable)
        guard isBreakActive else { return }
        if shouldEnable {
            startPlayback()
        } else {
            stopPlayback()
            restoreVolume()
        }
    }

    func selectPlaylist(at index: Int) {
        guard settings.selectPlaylist(at: index) else {
            NSLog("EduTools: ignored an invalid break music playlist selection")
            return
        }
        guard isBreakActive, settings.isMusicEnabled else { return }
        stopPlayback()
        startPlayback(captureVolume: volumeSnapshot == nil)
    }

    func skipToNextTrack() {
        guard isBreakActive,
              settings.isMusicEnabled,
              playbackSource == .folder,
              let folderPlaylist
        else {
            return
        }
        audioPlayer?.delegate = nil
        audioPlayer?.stop()
        audioPlayer = nil
        _ = folderPlaylist.advance(userSkip: true)
        playCurrentFolderTrack()
    }

    private func startPlayback(captureVolume: Bool = true) {
        switch settings.source {
        case .youtube:
            startYouTubePlayback(captureVolume: captureVolume)
        case .folder:
            startFolderPlayback()
        }
    }

    private func startYouTubePlayback(captureVolume: Bool) {
        guard activePlayback == nil else { return }
        guard let playlistURL = settings.selectedPlaylistURL else {
            NSLog("EduTools: break music is enabled but no valid playlist URL is configured")
            return
        }

        if captureVolume {
            volumeSnapshot = volumeController.captureDefaultOutputVolume()
            isFadeAvailable = volumeSnapshot != nil
            if volumeSnapshot == nil, settings.isFadeEnabled {
                NSLog("EduTools: break music fade is unavailable for the default output device")
            }
        }

        let token = UUID()
        playbackSource = .youtube
        let shouldUseChrome = settings.browser != .defaultBrowser
        activePlayback = ActiveBreakMusicPlayback(
            token: token,
            url: playlistURL,
            usesChrome: shouldUseChrome,
            launchState: .opening,
            chromeTabReference: nil
        )

        if shouldUseChrome,
           let chromeURL = workspace.urlForApplication(withBundleIdentifier: "com.google.Chrome") {
            openWithChrome(
                playlistURL,
                applicationURL: chromeURL,
                appMode: settings.browser == .youtubeMusicApp,
                token: token
            )
        } else {
            if shouldUseChrome {
                NSLog("EduTools: Google Chrome is not installed; opening break music in the default browser")
            }
            updatePlaybackMode(token: token, usesChrome: false)
            openWithDefaultBrowser(playlistURL, token: token)
        }
    }

    private func startFolderPlayback() {
        guard folderScanToken == nil, folderPlaylist == nil, audioPlayer == nil else { return }
        guard let root = settings.musicFolderURL else {
            showMessage?("재생할 음악 파일이 없습니다")
            return
        }

        let token = UUID()
        let collector = playlistCollector
        let order = settings.order
        let repeatMode = settings.repeatMode
        playbackSource = .folder
        folderScanToken = token
        folderVolume = 1
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let result = collector.collect(root: root)
            DispatchQueue.main.async {
                guard let self,
                      self.folderScanToken == token,
                      self.isBreakActive,
                      self.settings.isMusicEnabled,
                      self.settings.source == .folder
                else {
                    return
                }
                self.folderScanToken = nil
                guard !result.tracks.isEmpty else {
                    self.playbackSource = nil
                    self.showMessage?("재생할 음악 파일이 없습니다")
                    return
                }
                self.folderPlaylist = MusicPlaylist(
                    tracks: result.tracks,
                    order: order,
                    repeat: repeatMode
                )
                self.playCurrentFolderTrack()
            }
        }
    }

    private func playCurrentFolderTrack() {
        guard playbackSource == .folder, let folderPlaylist else { return }
        while let track = folderPlaylist.current {
            do {
                let player = try AVAudioPlayer(contentsOf: track.url)
                player.delegate = self
                player.volume = folderVolume
                guard player.prepareToPlay() else {
                    throw NSError(
                        domain: "kr.datasci.edutools.break-music",
                        code: 1,
                        userInfo: [NSLocalizedDescriptionKey: "오디오 재생을 시작하지 못했습니다"]
                    )
                }
                audioPlayer = player
                guard player.play() else {
                    audioPlayer = nil
                    player.delegate = nil
                    throw NSError(
                        domain: "kr.datasci.edutools.break-music",
                        code: 1,
                        userInfo: [NSLocalizedDescriptionKey: "오디오 재생을 시작하지 못했습니다"]
                    )
                }
                folderPlaylist.reportSuccess()
                bringBreakScreenToFront?()
                return
            } catch {
                NSLog(
                    "EduTools: break music file could not be played at %@: %@",
                    track.url.path,
                    String(describing: error)
                )
                folderPlaylist.reportFailure()
                if folderPlaylist.exhausted {
                    folderPlaybackDidExhaust()
                    return
                }
                _ = folderPlaylist.advance(userSkip: true)
            }
        }
    }

    private func folderTrackDidFail(_ player: AVAudioPlayer, error: Error?) {
        guard player === audioPlayer, let folderPlaylist else { return }
        player.delegate = nil
        player.stop()
        audioPlayer = nil
        NSLog(
            "EduTools: break music playback failed: %@",
            error.map { String(describing: $0) } ?? "decoder stopped"
        )
        folderPlaylist.reportFailure()
        if folderPlaylist.exhausted {
            folderPlaybackDidExhaust()
            return
        }
        _ = folderPlaylist.advance(userSkip: true)
        playCurrentFolderTrack()
    }

    private func folderPlaybackDidExhaust() {
        audioPlayer?.delegate = nil
        audioPlayer?.stop()
        audioPlayer = nil
        folderPlaylist = nil
        folderScanToken = nil
        playbackSource = nil
        showMessage?("음악을 재생하지 못했습니다")
    }

    private func stopPlayback() {
        stopFolderPlayback()
        stopYouTubePlayback()
        playbackSource = nil
    }

    private func stopFolderPlayback() {
        folderScanToken = nil
        audioPlayer?.delegate = nil
        audioPlayer?.stop()
        audioPlayer = nil
        folderPlaylist = nil
        folderVolume = 1
    }

    func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool) {
        guard player === audioPlayer, let folderPlaylist else { return }
        guard flag else {
            folderTrackDidFail(player, error: nil)
            return
        }
        player.delegate = nil
        audioPlayer = nil
        folderPlaylist.reportSuccess()
        _ = folderPlaylist.advance(userSkip: false)
        playCurrentFolderTrack()
    }

    func audioPlayerDecodeErrorDidOccur(_ player: AVAudioPlayer, error: Error?) {
        folderTrackDidFail(player, error: error)
    }

    private func openWithChrome(
        _ url: URL,
        applicationURL: URL,
        appMode: Bool,
        token: UUID
    ) {
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = true
        if appMode {
            configuration.arguments = ["--app=\(url.absoluteString)"]
        }
        workspace.open(
            [url],
            withApplicationAt: applicationURL,
            configuration: configuration
        ) { [weak self] _, error in
            DispatchQueue.main.async {
                guard let self else { return }
                if let error {
                    NSLog(
                        "EduTools: Chrome could not open break music; using the default browser: %@",
                        String(describing: error)
                    )
                    guard self.isCurrentPlayback(token: token) else { return }
                    self.updatePlaybackMode(token: token, usesChrome: false)
                    self.openWithDefaultBrowser(url, token: token)
                    return
                }
                let tabReference = self.captureActiveChromeTab(openedAt: url)
                self.playbackDidOpen(
                    token: token,
                    url: url,
                    usesChrome: true,
                    chromeTabReference: tabReference
                )
            }
        }
    }

    private func openWithDefaultBrowser(_ url: URL, token: UUID) {
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = true
        workspace.open(url, configuration: configuration) { [weak self] _, error in
            DispatchQueue.main.async {
                guard let self else { return }
                if let error {
                    NSLog("EduTools: the default browser could not open break music: %@", String(describing: error))
                    if self.isCurrentPlayback(token: token) {
                        self.activePlayback = nil
                        self.playbackSource = nil
                        self.restoreVolume()
                        self.bringBreakScreenToFront?()
                    }
                    return
                }
                self.playbackDidOpen(
                    token: token,
                    url: url,
                    usesChrome: false,
                    chromeTabReference: nil
                )
            }
        }
    }

    private func playbackDidOpen(
        token: UUID,
        url: URL,
        usesChrome: Bool,
        chromeTabReference: ChromeTabReference?
    ) {
        guard isCurrentPlayback(token: token), isBreakActive, settings.isMusicEnabled else {
            if activePlayback != nil, chromeTabReference == nil {
                NSLog("EduTools: an obsolete break music launch could not be identified without risking the current playback")
                return
            }
            stopOpenedPlayback(
                url: url,
                usesChrome: usesChrome,
                chromeTabReference: chromeTabReference
            )
            return
        }
        activePlayback?.launchState = .opened
        activePlayback?.chromeTabReference = chromeTabReference
        bringBreakScreenToFront?()
    }

    private func stopYouTubePlayback() {
        guard let playback = activePlayback else { return }
        activePlayback = nil
        guard playback.launchState == .opened else { return }
        stopOpenedPlayback(
            url: playback.url,
            usesChrome: playback.usesChrome,
            chromeTabReference: playback.chromeTabReference
        )
    }

    private func stopOpenedPlayback(
        url: URL,
        usesChrome: Bool,
        chromeTabReference: ChromeTabReference?
    ) {
        if usesChrome {
            guard let chromeTabReference else {
                NSLog("EduTools: could not identify the Chrome tab opened for break music; sending the play/pause media key")
                sendPlayPauseMediaKey()
                return
            }
            if !closeChromeTab(chromeTabReference) {
                NSLog("EduTools: Chrome tab close failed; sending the play/pause media key")
                sendPlayPauseMediaKey()
            }
        } else {
            NSLog("EduTools: break music used the default browser; sending the play/pause media key")
            sendPlayPauseMediaKey()
        }
    }

    private func captureActiveChromeTab(openedAt url: URL) -> ChromeTabReference? {
        let targetURL = AppleScriptString.literal(url.absoluteString)
        let source = """
        set targetURL to \(targetURL)
        tell application id "com.google.Chrome"
            if (count of windows) is 0 then return missing value
            set candidateWindow to front window
            set candidateTab to active tab of candidateWindow
            if (URL of candidateTab as text) is not targetURL then return missing value
            return {id of candidateWindow, id of candidateTab}
        end tell
        """
        guard let script = NSAppleScript(source: source) else {
            NSLog("EduTools: could not construct the Chrome tab identification AppleScript")
            return nil
        }
        var error: NSDictionary?
        let result = script.executeAndReturnError(&error)
        if let error {
            NSLog("EduTools: Chrome tab identification AppleScript failed: %@", String(describing: error))
            return nil
        }
        guard result.numberOfItems == 2,
              let windowDescriptor = result.atIndex(1),
              let tabDescriptor = result.atIndex(2)
        else {
            NSLog("EduTools: Chrome did not report the newly opened break music tab")
            return nil
        }
        return ChromeTabReference(
            windowID: windowDescriptor.int32Value,
            tabID: tabDescriptor.int32Value,
            url: url
        )
    }

    private func closeChromeTab(_ reference: ChromeTabReference) -> Bool {
        guard !NSRunningApplication.runningApplications(
            withBundleIdentifier: "com.google.Chrome"
        ).isEmpty else {
            NSLog("EduTools: Chrome is no longer running; no break music tab remains to close")
            return true
        }

        let targetURL = AppleScriptString.literal(reference.url.absoluteString)
        let source = """
        set targetURL to \(targetURL)
        tell application id "com.google.Chrome"
            repeat with windowIndex from (count of windows) to 1 by -1
                set candidateWindow to window windowIndex
                if (id of candidateWindow) is \(reference.windowID) then
                    repeat with tabIndex from (count of tabs of candidateWindow) to 1 by -1
                        set candidateTab to tab tabIndex of candidateWindow
                        if (id of candidateTab) is \(reference.tabID) and (URL of candidateTab as text) is targetURL then
                            close candidateTab
                            return true
                        end if
                    end repeat
                end if
            end repeat
        end tell
        return false
        """
        guard let script = NSAppleScript(source: source) else {
            NSLog("EduTools: could not construct the Chrome tab close AppleScript")
            return false
        }
        var error: NSDictionary?
        let result = script.executeAndReturnError(&error)
        if let error {
            NSLog("EduTools: Chrome tab close AppleScript failed: %@", String(describing: error))
            return false
        }
        guard result.booleanValue else {
            NSLog("EduTools: Chrome did not contain the break music URL opened by EduTools")
            return false
        }
        return true
    }

    private func sendPlayPauseMediaKey() {
        let playPauseKeyType = 16
        let auxiliaryControlSubtype: Int16 = 8
        let keyDownState = 0xA
        let keyUpState = 0xB

        func post(state: Int) -> Bool {
            let data1 = (playPauseKeyType << 16) | (state << 8)
            guard let event = NSEvent.otherEvent(
                with: .systemDefined,
                location: .zero,
                modifierFlags: [],
                timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: 0,
                context: nil,
                subtype: auxiliaryControlSubtype,
                data1: data1,
                data2: -1
            )?.cgEvent else {
                return false
            }
            event.post(tap: .cghidEventTap)
            return true
        }

        guard post(state: keyDownState), post(state: keyUpState) else {
            NSLog("EduTools: could not create the play/pause media key event")
            return
        }
    }

    private func restoreVolume() {
        guard let volumeSnapshot else { return }
        self.volumeSnapshot = nil
        if !volumeController.setVolume(volumeSnapshot.volume, on: volumeSnapshot) {
            NSLog("EduTools: could not restore the system volume after break music")
        }
        isFadeAvailable = true
    }

    private func isCurrentPlayback(token: UUID) -> Bool {
        activePlayback?.token == token
    }

    private func updatePlaybackMode(token: UUID, usesChrome: Bool) {
        guard isCurrentPlayback(token: token) else { return }
        activePlayback?.usesChrome = usesChrome
    }
}
