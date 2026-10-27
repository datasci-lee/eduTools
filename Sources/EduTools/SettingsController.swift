import AppKit
import ApplicationServices
import Carbon
import ServiceManagement
import SwiftUI
import UniformTypeIdentifiers

private struct SettingsActions {
    let updateLaunchAtLogin: (Bool) -> Void
    let chooseArchiveFolder: () -> Void
    let editPlaylists: () -> Void
    let exportSettings: () -> Void
    let importSettings: () -> Void
    let resetSettings: () -> Void
    let restorePrevious: () -> Void
}

final class SettingsController: NSObject {
    private enum DefaultsKey {
        static let didShowPermissionGuide = "EduTools.permissions.didShowGuide.v1"
    }

    private let settingsStore: SettingsStore
    private let backupStore: SettingsBackupStore
    private let defaults: UserDefaults
    private var settingsWindow: NSWindow?
    private var importWindow: NSWindow?
    private var permissionWindow: NSWindow?

    init(settingsStore: SettingsStore, defaults: UserDefaults = .standard) {
        self.settingsStore = settingsStore
        backupStore = SettingsBackupStore(defaults: defaults)
        self.defaults = defaults
        super.init()
    }

    func showSettings() {
        if let settingsWindow {
            NSApp.activate()
            settingsWindow.makeKeyAndOrderFront(nil)
            return
        }

        let actions = SettingsActions(
            updateLaunchAtLogin: { [weak self] enabled in
                self?.updateLaunchAtLogin(enabled)
            },
            chooseArchiveFolder: { [weak self] in self?.chooseArchiveFolder() },
            editPlaylists: { [weak self] in self?.editPlaylists() },
            exportSettings: { [weak self] in self?.exportSettings() },
            importSettings: { [weak self] in self?.chooseSettingsToImport() },
            resetSettings: { [weak self] in self?.confirmResetToDefaults() },
            restorePrevious: { [weak self] in self?.restorePreviousSettings() }
        )
        let rootView = SettingsRootView(store: settingsStore, actions: actions)
        let hostingController = NSHostingController(rootView: rootView)
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 780, height: 660),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "EduTools 설정"
        window.contentViewController = hostingController
        window.center()
        window.isReleasedWhenClosed = false
        window.setFrameAutosaveName("EduTools.SettingsWindow")
        settingsWindow = window
        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
    }

    func refreshShortcutPresentation() {
        settingsStore.objectWillChange.send()
    }

    func exportSettings() {
        let accessory = SettingsExportOptionsView()
        let alert = NSAlert()
        alert.messageText = "내보낼 설정 묶음"
        alert.informativeText = "이 컴퓨터 전용 항목은 기본적으로 제외됩니다."
        alert.accessoryView = accessory
        alert.addButton(withTitle: "계속")
        alert.addButton(withTitle: "취소")
        guard alert.runModal() == .alertFirstButtonReturn else { return }

        let bundles = accessory.selectedBundles
        guard !bundles.isEmpty else {
            presentError("내보낼 설정 묶음을 하나 이상 선택하세요.")
            return
        }
        if bundles.contains(.timer), !settingsStore.settings.timer.playlistURLs.isEmpty {
            let playlistAlert = NSAlert()
            playlistAlert.alertStyle = .warning
            playlistAlert.messageText = "재생목록 주소가 포함됩니다"
            playlistAlert.informativeText = "설정 파일을 공유하면 YouTube Music 재생목록 URL도 함께 전달됩니다."
            playlistAlert.addButton(withTitle: "포함하여 계속")
            playlistAlert.addButton(withTitle: "취소")
            guard playlistAlert.runModal() == .alertFirstButtonReturn else { return }
        }

        let panel = NSSavePanel()
        panel.title = "설정 내보내기"
        panel.nameFieldStringValue = "EduTools-settings.pansetting"
        panel.allowedContentTypes = [UTType(exportedAs: SettingsDocument.format)]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        do {
            let document = try SettingsTransfer.exportDocument(
                settings: settingsStore.settings,
                bundles: bundles,
                includeComputerSpecific: accessory.includesComputerSpecific
            )
            try SettingsTransfer.encode(document).write(to: url, options: .atomic)
        } catch {
            NSLog("EduTools: settings export failed: %@", String(describing: error))
            presentError(error.localizedDescription)
        }
    }

    func chooseSettingsToImport() {
        let panel = NSOpenPanel()
        panel.title = "설정 가져오기"
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.allowedContentTypes = [UTType(exportedAs: SettingsDocument.format)]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        openImportPreview(for: url)
    }

    func openImportPreview(for url: URL) {
        do {
            let document = try SettingsTransfer.decode(Data(contentsOf: url))
            showImportPreview(document)
        } catch {
            NSLog("EduTools: settings import validation failed: %@", String(describing: error))
            presentError(error.localizedDescription)
        }
    }

    func confirmResetToDefaults() {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "모든 설정을 기본값으로 초기화할까요?"
        alert.informativeText = "현재 설정은 되돌릴 수 있도록 자동 백업됩니다."
        alert.addButton(withTitle: "초기화")
        alert.addButton(withTitle: "취소")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        do {
            try backupStore.append(settingsStore.settings)
            settingsStore.resetToDefaults()
            synchronizeLaunchAtLogin(with: false)
        } catch {
            NSLog("EduTools: settings reset backup failed: %@", String(describing: error))
            presentError(error.localizedDescription)
        }
    }

    func showFirstRunPermissionsIfNeeded() {
        guard !defaults.bool(forKey: DefaultsKey.didShowPermissionGuide) else { return }
        defaults.set(true, forKey: DefaultsKey.didShowPermissionGuide)
        showPermissionGuide()
    }

    func showVoiceOverWarningIfNeeded() {
        guard NSWorkspace.shared.isVoiceOverEnabled,
              settingsStore.settings.shortcuts.appModifier == .controlOption
        else {
            return
        }
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "VoiceOver 단축키와 겹칩니다"
        alert.informativeText = "VoiceOver가 ⌃⌥를 사용 중입니다. EduTools 앱 수정키를 ⌃⌥⇧로 바꾸는 것을 권장합니다."
        alert.addButton(withTitle: "⌃⌥⇧로 변경")
        alert.addButton(withTitle: "유지")
        if alert.runModal() == .alertFirstButtonReturn {
            settingsStore.update { $0.shortcuts.appModifier = .controlOptionShift }
        }
    }

    private func updateLaunchAtLogin(_ enabled: Bool) {
        synchronizeLaunchAtLogin(with: enabled)
    }

    private func synchronizeLaunchAtLogin(with enabled: Bool) {
        Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                if enabled, SMAppService.mainApp.status != .enabled {
                    try SMAppService.mainApp.register()
                } else if !enabled, SMAppService.mainApp.status != .notRegistered {
                    try await SMAppService.mainApp.unregister()
                }
                settingsStore.update { $0.general.launchAtLogin = enabled }
            } catch {
                NSLog("EduTools: failed to update launch-at-login: %@", String(describing: error))
                presentError("로그인 시 자동 실행을 변경하지 못했습니다: \(error.localizedDescription)")
                settingsStore.objectWillChange.send()
            }
        }
    }

    private func chooseArchiveFolder() {
        let panel = NSOpenPanel()
        panel.title = "자동 보관 폴더 선택"
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        settingsStore.update { $0.general.archiveFolderPath = url.path }
    }

    private func editPlaylists() {
        let alert = NSAlert()
        alert.messageText = "YouTube Music 재생목록"
        alert.informativeText = "한 줄에 하나씩 최대 5개의 https://music.youtube.com/ 주소를 입력하세요."
        let scrollView = NSScrollView(frame: NSRect(x: 0, y: 0, width: 460, height: 150))
        let textView = NSTextView(frame: scrollView.bounds)
        textView.string = settingsStore.settings.timer.playlistURLs.joined(separator: "\n")
        scrollView.documentView = textView
        scrollView.hasVerticalScroller = true
        alert.accessoryView = scrollView
        alert.addButton(withTitle: "적용")
        alert.addButton(withTitle: "취소")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let values = textView.string.split(whereSeparator: \.isNewline).map {
            $0.trimmingCharacters(in: .whitespacesAndNewlines)
        }.filter { !$0.isEmpty }
        guard values.count <= BreakMusicSettings.maximumPlaylistCount,
              values.allSatisfy({ BreakMusicURL.validated($0) != nil })
        else {
            presentError("재생목록 주소 형식 또는 최대 5개 제한을 확인하세요.")
            return
        }
        settingsStore.update {
            $0.timer.playlistURLs = values
            $0.timer.defaultPlaylistIndex = values.isEmpty
                ? 0
                : min($0.timer.defaultPlaylistIndex, values.count - 1)
        }
    }

    private func restorePreviousSettings() {
        do {
            guard let settings = try backupStore.popLatest() else {
                presentError("되돌릴 이전 설정이 없습니다.")
                return
            }
            try settingsStore.apply(settings)
            synchronizeLaunchAtLogin(with: settings.general.launchAtLogin)
        } catch {
            NSLog("EduTools: settings restore failed: %@", String(describing: error))
            presentError(error.localizedDescription)
        }
    }

    private func showImportPreview(_ document: SettingsDocument) {
        let view = SettingsImportPreviewView(
            store: settingsStore,
            document: document,
            apply: { [weak self] selectedBundles in
                self?.applyImportedSettings(document, selectedBundles: selectedBundles)
            },
            cancel: { [weak self] in self?.importWindow?.close() }
        )
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 760, height: 600),
            styleMask: [.titled, .closable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "설정 가져오기 미리보기"
        window.contentViewController = NSHostingController(rootView: view)
        window.center()
        window.isReleasedWhenClosed = false
        importWindow?.close()
        importWindow = window
        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
    }

    private func applyImportedSettings(
        _ document: SettingsDocument,
        selectedBundles: Set<SettingsBundle>
    ) {
        do {
            let imported = try SettingsTransfer.appliedSettings(
                current: settingsStore.settings,
                document: document,
                selectedBundles: selectedBundles
            )
            try backupStore.append(settingsStore.settings)
            try settingsStore.apply(imported)
            synchronizeLaunchAtLogin(with: imported.general.launchAtLogin)
            importWindow?.close()
        } catch {
            NSLog("EduTools: settings import apply failed: %@", String(describing: error))
            presentError(error.localizedDescription)
        }
    }

    private func showPermissionGuide() {
        let view = PermissionGuideView(
            openSystemSettings: { [weak self] anchor in
                self?.openSystemSettings(anchor: anchor)
            },
            close: { [weak self] in self?.permissionWindow?.close() }
        )
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 620, height: 480),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "EduTools 권한 안내"
        window.contentViewController = NSHostingController(rootView: view)
        window.center()
        window.isReleasedWhenClosed = false
        permissionWindow = window
        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
    }

    private func openSystemSettings(anchor: String) {
        guard let url = URL(
            string: "x-apple.systempreferences:com.apple.preference.security?\(anchor)"
        ) else {
            NSLog("EduTools: invalid System Settings permission anchor: %@", anchor)
            return
        }
        if !NSWorkspace.shared.open(url) {
            NSLog("EduTools: failed to open System Settings permission anchor: %@", anchor)
            presentError("시스템 설정을 열지 못했습니다.")
        }
    }

    private func presentError(_ message: String) {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "EduTools 설정"
        alert.informativeText = message
        alert.addButton(withTitle: "확인")
        alert.runModal()
    }
}

private final class SettingsExportOptionsView: NSStackView {
    private var bundleButtons: [SettingsBundle: NSButton] = [:]
    private let computerSpecificButton = NSButton(checkboxWithTitle: "이 컴퓨터 전용 항목 포함", target: nil, action: nil)

    override init(frame frameRect: NSRect) {
        super.init(frame: NSRect(x: 0, y: 0, width: 360, height: 250))
        orientation = .vertical
        alignment = .leading
        spacing = 5
        for bundle in SettingsBundle.allCases {
            let button = NSButton(checkboxWithTitle: bundle.displayName, target: nil, action: nil)
            button.state = .on
            bundleButtons[bundle] = button
            addArrangedSubview(button)
        }
        computerSpecificButton.state = .off
        addArrangedSubview(computerSpecificButton)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }

    var selectedBundles: Set<SettingsBundle> {
        Set(bundleButtons.compactMap { $0.value.state == .on ? $0.key : nil })
    }

    var includesComputerSpecific: Bool {
        computerSpecificButton.state == .on
    }
}

private struct SettingsRootView: View {
    @ObservedObject var store: SettingsStore
    let actions: SettingsActions

    var body: some View {
        TabView {
            generalTab.tabItem { Label("일반", systemImage: "gearshape") }
            toolsTab.tabItem { Label("도구", systemImage: "pencil.and.outline") }
            eraserTab.tabItem { Label("지우개", systemImage: "eraser") }
            zoomTab.tabItem { Label("줌", systemImage: "plus.magnifyingglass") }
            boardTab.tabItem { Label("보드", systemImage: "rectangle.fill") }
            timerTab.tabItem { Label("타이머", systemImage: "timer") }
            shortcutsTab.tabItem { Label("단축키", systemImage: "keyboard") }
        }
        .padding(14)
        .frame(minWidth: 720, minHeight: 590)
    }

    private func binding<Value>(_ keyPath: WritableKeyPath<AppSettings, Value>) -> Binding<Value> {
        Binding(
            get: { store.settings[keyPath: keyPath] },
            set: { value in store.update { $0[keyPath: keyPath] = value } }
        )
    }

    private var generalTab: some View {
        Form {
            Toggle(
                "로그인 시 자동 실행",
                isOn: Binding(
                    get: { store.settings.general.launchAtLogin },
                    set: actions.updateLaunchAtLogin
                )
            )
            Toggle(
                "판서 모드를 끌 때 내용 삭제",
                isOn: binding(\.general.clearsInkWhenAnnotationModeEnds)
            )
            Toggle("지우기 전 자동 보관", isOn: binding(\.general.autoArchiveEnabled))
            HStack {
                Text("자동 보관 폴더")
                Spacer()
                Text(store.settings.general.archiveFolderPath ?? "기본 폴더")
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Button("선택…", action: actions.chooseArchiveFolder)
            }
            Toggle(
                "도움말을 화면 공유·녹화에 표시",
                isOn: binding(\.help.showsHelpInSharing)
            )
            Divider()
            HStack {
                Button("내보내기…", action: actions.exportSettings)
                Button("가져오기…", action: actions.importSettings)
                Button("이전 설정으로 되돌리기", action: actions.restorePrevious)
                Spacer()
                Button("기본값으로 초기화", action: actions.resetSettings)
            }
        }
        .padding()
    }

    private var toolsTab: some View {
        Form {
            Section("색상 프리셋") {
                HStack {
                    ForEach(store.settings.tools.colors.indices, id: \.self) { index in
                        ColorPicker(
                            "\(index + 1)",
                            selection: colorBinding(\.tools.colors, index: index),
                            supportsOpacity: false
                        )
                        .labelsHidden()
                    }
                }
            }
            slider("펜 기본 굵기", keyPath: \.tools.penWidth, range: 1...30, step: 1)
            slider("형광펜 기본 굵기", keyPath: \.tools.highlighterWidth, range: 1...30, step: 1)
            slider("형광펜 투명도", keyPath: \.tools.highlighterOpacity, range: 0.05...1, step: 0.05)
            ColorPicker("레이저 색", selection: colorBinding(\.tools.laserColor), supportsOpacity: false)
            slider("레이저 크기", keyPath: \.tools.laserSize, range: 2...50, step: 1)
            slider("레이저 사라지는 시간", keyPath: \.tools.laserTrailDuration, range: 0.1...10, step: 0.1)
            Toggle("도형 자동 보정", isOn: binding(\.tools.shapeRecognitionEnabled))
        }
        .padding()
    }

    private var eraserTab: some View {
        Form {
            Picker("기본 방식", selection: binding(\.eraser.mode)) {
                Text("부분").tag("partial")
                Text("획").tag("stroke")
                Text("영역").tag("area")
            }
            .pickerStyle(.segmented)
            slider("기본 크기", keyPath: \.eraser.diameter, range: 8...200, step: 1)
            ForEach(0..<3, id: \.self) { index in
                slider(
                    "크기 프리셋 \(index + 1)",
                    binding: arrayBinding(\.eraser.presetDiameters, index: index),
                    range: 8...200,
                    step: 1
                )
            }
            Picker("판정 기준", selection: binding(\.eraser.hitRule)) {
                Text("닿으면").tag("touching")
                Text("완전히 포함").tag("contained")
            }
            Picker("대상", selection: binding(\.eraser.targetFilter)) {
                Text("전체").tag("all")
                Text("펜만").tag("penOnly")
                Text("형광펜만").tag("highlighterOnly")
            }
        }
        .padding()
    }

    private var zoomTab: some View {
        Form {
            slider("시작 배율", keyPath: \.zoom.startingMagnification, range: 1.25...8, step: 0.25)
            slider("최대 배율", keyPath: \.zoom.maximumMagnification, range: 1.25...8, step: 0.25)
            Picker("배율 단계", selection: binding(\.zoom.magnificationStep)) {
                Text("0.25배").tag(0.25)
                Text("0.5배").tag(0.5)
            }
            Picker("따라가기", selection: binding(\.zoom.followMode)) {
                Text("항상").tag("always")
                Text("가장자리").tag("edge")
                Text("고정").tag("fixed")
            }
            slider("따라가기 부드러움", keyPath: \.zoom.followSmoothness, range: 0...1, step: 0.05)
            Toggle("라이브 줌을 기본값으로", isOn: binding(\.zoom.liveByDefault))
        }
        .padding()
    }

    private var boardTab: some View {
        Form {
            ColorPicker("화이트보드 배경", selection: colorBinding(\.board.whiteboardColor))
            ColorPicker("블랙보드 배경", selection: colorBinding(\.board.blackboardColor))
            Picker("배경 무늬", selection: binding(\.board.backgroundPattern)) {
                Text("없음").tag("none")
                Text("격자").tag("grid")
                Text("줄").tag("lines")
            }
            .pickerStyle(.segmented)
        }
        .padding()
    }

    private var timerTab: some View {
        Form {
            Section("수업") {
                ForEach(store.settings.timer.lessonPresets.indices, id: \.self) { index in
                    Stepper(
                        "프리셋 \(index + 1): \(store.settings.timer.lessonPresets[index])분",
                        value: intArrayBinding(\.timer.lessonPresets, index: index),
                        in: 1...240
                    )
                }
                Stepper(
                    "기본 수업 시간: \(store.settings.timer.lessonDefaultMinutes)분",
                    value: binding(\.timer.lessonDefaultMinutes),
                    in: 1...240
                )
                Picker("알림 사운드", selection: binding(\.timer.notificationSound)) {
                    Text("Glass").tag("Glass")
                    Text("Ping").tag("Ping")
                    Text("Pop").tag("Pop")
                }
                Toggle("수업 종료 후 쉬는 시간 자동 시작", isOn: binding(\.timer.autoStartBreak))
            }
            Section("쉬는 시간") {
                Stepper(
                    "기본 길이: \(store.settings.timer.breakDefaultMinutes)분",
                    value: binding(\.timer.breakDefaultMinutes),
                    in: 1...180
                )
                TextField("화면 문구", text: binding(\.timer.breakMessage))
                ColorPicker("배경색", selection: colorBinding(\.timer.breakBackgroundColor))
                HStack {
                    Text("음악 재생목록 \(store.settings.timer.playlistURLs.count)개")
                    Spacer()
                    Button("편집…", action: actions.editPlaylists)
                }
                Picker("열 브라우저", selection: binding(\.timer.musicBrowser)) {
                    Text("Chrome").tag("chrome")
                    Text("YouTube Music 앱").tag("youtubeMusicApp")
                    Text("기본 브라우저").tag("defaultBrowser")
                }
                Toggle("종료 전 페이드아웃", isOn: binding(\.timer.fadeEnabled))
                slider("페이드 시간(초)", keyPath: \.timer.fadeDuration, range: 1...600, step: 1)
                Text("YouTube Premium이 아니면 재생 중 광고가 나올 수 있습니다.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .padding()
    }

    private var shortcutsTab: some View {
        VStack(alignment: .leading, spacing: 10) {
            if NSWorkspace.shared.isVoiceOverEnabled,
               store.settings.shortcuts.appModifier == .controlOption
            {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.yellow)
                    Text("VoiceOver가 ⌃⌥를 사용 중입니다. 앱 수정키를 ⌃⌥⇧로 바꾸세요.")
                    Spacer()
                    Button("변경") {
                        store.update { $0.shortcuts.appModifier = .controlOptionShift }
                    }
                }
                .padding(8)
                .background(.yellow.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
            }
            Picker("앱 수정키", selection: binding(\.shortcuts.appModifier)) {
                ForEach(AppModifierSetting.allCases, id: \.self) { modifier in
                    Text(modifier.displayName).tag(modifier)
                }
            }
            HStack {
                Text("항목을 클릭한 뒤 새 조합을 누르세요. 물리 키 위치로 저장됩니다.")
                    .foregroundStyle(.secondary)
                Spacer()
                Button("단축키 기본값 복원") {
                    store.update { $0.shortcuts.overrides.removeAll() }
                }
            }
            List(shortcutRows, id: \.key) { row in
                ShortcutSettingRow(store: store, row: row)
            }
        }
        .padding()
    }

    private var shortcutRows: [ShortcutRowModel] {
        ShortcutCatalog.definitions.flatMap { definition in
            definition.bindings.map { binding in
                ShortcutRowModel(
                    key: ShortcutCatalog.overrideKey(
                        definitionID: definition.id,
                        scope: binding.scope
                    ),
                    name: definition.name,
                    scope: binding.scope,
                    defaultPortable: binding.portableString
                )
            }
        }
    }

    private func slider(
        _ title: String,
        keyPath: WritableKeyPath<AppSettings, Double>,
        range: ClosedRange<Double>,
        step: Double
    ) -> some View {
        slider(title, binding: binding(keyPath), range: range, step: step)
    }

    private func slider(
        _ title: String,
        binding: Binding<Double>,
        range: ClosedRange<Double>,
        step: Double
    ) -> some View {
        HStack {
            Text(title)
            Slider(value: binding, in: range, step: step)
            Text(binding.wrappedValue.formatted(.number.precision(.fractionLength(0...2))))
                .monospacedDigit()
                .frame(width: 54, alignment: .trailing)
        }
    }

    private func colorBinding(
        _ keyPath: WritableKeyPath<AppSettings, SettingsColor>,
        supportsOpacity: Bool = true
    ) -> Binding<Color> {
        Binding(
            get: { Color(store.settings[keyPath: keyPath].nsColor) },
            set: { value in
                store.update { $0[keyPath: keyPath] = SettingsColor(NSColor(value)) }
            }
        )
    }

    private func colorBinding(
        _ keyPath: WritableKeyPath<AppSettings, [SettingsColor]>,
        index: Int
    ) -> Binding<Color> {
        Binding(
            get: { Color(store.settings[keyPath: keyPath][index].nsColor) },
            set: { value in
                store.update { $0[keyPath: keyPath][index] = SettingsColor(NSColor(value)) }
            }
        )
    }

    private func arrayBinding(
        _ keyPath: WritableKeyPath<AppSettings, [Double]>,
        index: Int
    ) -> Binding<Double> {
        Binding(
            get: { store.settings[keyPath: keyPath][index] },
            set: { value in store.update { $0[keyPath: keyPath][index] = value } }
        )
    }

    private func intArrayBinding(
        _ keyPath: WritableKeyPath<AppSettings, [Int]>,
        index: Int
    ) -> Binding<Int> {
        Binding(
            get: { store.settings[keyPath: keyPath][index] },
            set: { value in store.update { $0[keyPath: keyPath][index] = value } }
        )
    }
}

private struct ShortcutRowModel {
    let key: String
    let name: String
    let scope: ShortcutScope
    let defaultPortable: String
}

private struct ShortcutSettingRow: View {
    @ObservedObject var store: SettingsStore
    let row: ShortcutRowModel

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack {
                Text(row.name)
                Text(scopeName).font(.caption).foregroundStyle(.secondary)
                Spacer()
                ShortcutRecorderView(
                    scope: row.scope,
                    appModifier: store.settings.shortcuts.appModifier,
                    portableString: portableString
                ) { value in
                    store.update { $0.shortcuts.overrides[row.key] = value }
                }
                .frame(width: 150, height: 25)
            }
            if let warning = warning {
                Text("⚠ \(warning)").font(.caption).foregroundStyle(.orange)
            }
        }
        .padding(.vertical, 2)
    }

    private var portableString: String {
        store.settings.shortcuts.overrides[row.key] ?? row.defaultPortable
    }

    private var warning: String? {
        store.runtimeShortcutWarnings[row.key]
            ?? ShortcutConflictDetector.warnings(for: store.settings.shortcuts)[row.key]
    }

    private var scopeName: String {
        switch row.scope {
        case .global: return "전역"
        case .annotation: return "판서"
        case .zoom: return "줌"
        }
    }
}

private struct ShortcutRecorderView: NSViewRepresentable {
    let scope: ShortcutScope
    let appModifier: AppModifierSetting
    let portableString: String
    let onRecord: (String) -> Void

    func makeNSView(context: Context) -> ShortcutRecorderButton {
        let button = ShortcutRecorderButton(title: "", target: nil, action: nil)
        button.bezelStyle = .rounded
        return button
    }

    func updateNSView(_ button: ShortcutRecorderButton, context: Context) {
        button.scope = scope
        button.appModifier = appModifier
        button.portableString = portableString
        button.onRecord = onRecord
        if !button.isRecording {
            button.title = PortableShortcut(serialized: portableString)?.displayString(
                appModifier: appModifier
            ) ?? portableString
        }
    }
}

private final class ShortcutRecorderButton: NSButton {
    var scope = ShortcutScope.annotation
    var appModifier = AppModifierSetting.controlOption
    var portableString = ""
    var onRecord: ((String) -> Void)?
    private(set) var isRecording = false

    override var acceptsFirstResponder: Bool { true }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        target = self
        action = #selector(beginRecording)
    }

    convenience init(title: String, target: AnyObject?, action: Selector?) {
        self.init(frame: .zero)
        self.title = title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }

    @objc private func beginRecording() {
        isRecording = true
        title = "키를 누르세요…"
        window?.makeFirstResponder(self)
    }

    override func keyDown(with event: NSEvent) {
        guard isRecording else {
            super.keyDown(with: event)
            return
        }
        if event.keyCode == UInt16(kVK_Escape) {
            finishRecording(with: nil)
            return
        }
        guard let token = PortableShortcut.keyToken(for: event.keyCode) else {
            NSSound.beep()
            return
        }
        let serialized: String?
        if scope == .global {
            let expected: NSEvent.ModifierFlags = appModifier == .controlOption
                ? [.control, .option]
                : [.control, .option, .shift]
            let relevant = event.modifierFlags.intersection([.command, .shift, .control, .option])
            guard relevant == expected else {
                NSSound.beep()
                return
            }
            serialized = "AppMod+\(token)"
        } else {
            serialized = PortableShortcut.serialized(
                keyCode: event.keyCode,
                modifiers: event.modifierFlags
            )
        }
        finishRecording(with: serialized)
    }

    private func finishRecording(with value: String?) {
        isRecording = false
        if let value {
            portableString = value
            onRecord?(value)
        }
        title = PortableShortcut(serialized: portableString)?.displayString(
            appModifier: appModifier
        ) ?? portableString
        window?.makeFirstResponder(nil)
    }
}

private struct SettingsImportPreviewView: View {
    @ObservedObject var store: SettingsStore
    let document: SettingsDocument
    let apply: (Set<SettingsBundle>) -> Void
    let cancel: () -> Void
    @State private var selectedBundles: Set<SettingsBundle>

    init(
        store: SettingsStore,
        document: SettingsDocument,
        apply: @escaping (Set<SettingsBundle>) -> Void,
        cancel: @escaping () -> Void
    ) {
        self.store = store
        self.document = document
        self.apply = apply
        self.cancel = cancel
        _selectedBundles = State(initialValue: Set(document.includedBundles))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("형식 버전 \(document.formatVersion) · 앱 \(document.appVersion) · \(document.createdOS)")
                .foregroundStyle(.secondary)
            HStack {
                ForEach(document.includedBundles, id: \.self) { bundle in
                    Toggle(
                        bundle.displayName,
                        isOn: Binding(
                            get: { selectedBundles.contains(bundle) },
                            set: { enabled in
                                if enabled { selectedBundles.insert(bundle) }
                                else { selectedBundles.remove(bundle) }
                            }
                        )
                    )
                    .toggleStyle(.checkbox)
                }
            }
            if !shortcutWarnings.isEmpty {
                Text("⚠ 단축키 충돌: \(shortcutWarnings.joined(separator: " · "))")
                    .foregroundStyle(.orange)
            }
            List(changes) { change in
                VStack(alignment: .leading, spacing: 2) {
                    Text(change.id).font(.headline)
                    Text("\(change.currentValue)  →  \(change.importedValue)")
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 3)
            }
            HStack {
                Text("적용 직전에 현재 설정을 자동 백업하며 최근 5개를 보관합니다.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                Spacer()
                Button("취소", action: cancel)
                Button("선택한 묶음 적용") { apply(selectedBundles) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(selectedBundles.isEmpty)
            }
        }
        .padding(18)
        .frame(minWidth: 700, minHeight: 520)
    }

    private var changes: [SettingsChange] {
        do {
            return try SettingsTransfer.changes(
                current: store.settings,
                document: document,
                selectedBundles: selectedBundles
            )
        } catch {
            NSLog("EduTools: failed to build settings import preview: %@", String(describing: error))
            return [SettingsChange(
                id: "가져오기 오류",
                currentValue: "—",
                importedValue: error.localizedDescription
            )]
        }
    }

    private var shortcutWarnings: [String] {
        guard selectedBundles.contains(.shortcuts) else {
            return []
        }
        do {
            let imported = try SettingsTransfer.appliedSettings(
                current: store.settings,
                document: document,
                selectedBundles: selectedBundles
            )
            return Array(Set(
                ShortcutConflictDetector.warnings(for: imported.shortcuts).values
            )).sorted()
        } catch {
            NSLog("EduTools: failed to check imported shortcuts: %@", String(describing: error))
            return [error.localizedDescription]
        }
    }
}

private struct PermissionGuideView: View {
    let openSystemSettings: (String) -> Void
    let close: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("기능별 권한 안내").font(.title2.bold())
            permissionRow(
                title: "화면 및 시스템 오디오 녹음",
                feature: "화면 줌, 화면 배경을 포함한 PNG 저장",
                unavailable: "없으면 해당 기능을 누를 때 권한 안내를 표시합니다.",
                anchor: "Privacy_ScreenCapture"
            )
            permissionRow(
                title: "손쉬운 사용",
                feature: "키 입력 표시, 클릭 강조, 슬라이드 넘김 키 전달",
                unavailable: "없으면 이 세 기능만 꺼지고 판서는 계속 동작합니다.",
                anchor: "Privacy_Accessibility"
            )
            permissionRow(
                title: "자동화",
                feature: "쉬는 시간 종료 시 EduTools가 연 Chrome 음악 탭 닫기",
                unavailable: "없으면 미디어 재생/일시정지 키로 대체합니다.",
                anchor: "Privacy_Automation"
            )
            Text("전역 단축키는 별도 권한이 필요 없습니다.")
                .font(.footnote)
                .foregroundStyle(.secondary)
            HStack {
                Spacer()
                Button("확인", action: close).keyboardShortcut(.defaultAction)
            }
        }
        .padding(22)
        .frame(minWidth: 560, minHeight: 400)
    }

    private func permissionRow(
        title: String,
        feature: String,
        unavailable: String,
        anchor: String
    ) -> some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.headline)
                Text("사용 기능: \(feature)")
                Text("권한이 없을 때: \(unavailable)").foregroundStyle(.secondary)
            }
            Spacer()
            Button("시스템 설정 열기") { openSystemSettings(anchor) }
        }
    }
}
