import SwiftUI

private enum PresetScreenSlot: Int, CaseIterable, Hashable {
    case main
    case secondary1
    case secondary2

    var title: String {
        switch self {
        case .main: return "주 화면"
        case .secondary1: return "보조화면1"
        case .secondary2: return "보조화면2"
        }
    }
}

private struct DisplaySettingsMessage: Identifiable {
    let id = UUID()
    var title: String
    var detail: String
}

struct DisplayPresetsSettingsView: View {
    @ObservedObject var store: SettingsStore

    @State private var selectedPresetID: UUID?
    @State private var onlineDisplays: [OnlineDisplay] = []
    @State private var renameDraft = ""
    @State private var isRenaming = false
    @State private var presentedMessage: DisplaySettingsMessage?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Toggle(
                    "연결된 모니터에 따라 자동 전환",
                    isOn: Binding(
                        get: { store.settings.display.autoSwitchEnabled },
                        set: { value in
                            store.update { $0.display.autoSwitchEnabled = value }
                        }
                    )
                )
                .disabled(true)
                Spacer()
                Button {
                    refreshOnlineDisplays(showingErrors: true)
                } label: {
                    Label("연결 상태 새로 고침", systemImage: "arrow.clockwise")
                }
            }

            VStack(alignment: .leading, spacing: 3) {
                Text("자동 집합이 같으면 목록의 위쪽 프리셋이 우선입니다.")
                if let currentAutoMatchDescription {
                    Text(currentAutoMatchDescription)
                }
            }
            .font(.callout)
            .foregroundStyle(.secondary)

            HSplitView {
                presetSidebar
                    .frame(minWidth: 210, idealWidth: 230, maxWidth: 270)
                Group {
                    if let preset = selectedPreset {
                        presetEditor(preset)
                    } else {
                        ContentUnavailableView(
                            "화면 구성 프리셋 없음",
                            systemImage: "display.2",
                            description: Text("현재 구성으로 프리셋을 추가하세요.")
                        )
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding()
        .onAppear {
            if selectedPresetID == nil {
                selectedPresetID = store.settings.display.presets.first?.id
            }
            refreshOnlineDisplays(showingErrors: true)
        }
        .alert(item: $presentedMessage) { message in
            Alert(
                title: Text(message.title),
                message: Text(message.detail),
                dismissButton: .default(Text("확인"))
            )
        }
        .alert("프리셋 이름 바꾸기", isPresented: $isRenaming) {
            TextField("이름", text: $renameDraft)
            Button("취소", role: .cancel) {}
            Button("저장") { commitRename() }
                .disabled(!isValidName(renameDraft))
        }
    }

    private var presetSidebar: some View {
        VStack(spacing: 8) {
            List(selection: $selectedPresetID) {
                ForEach(
                    Array(store.settings.display.presets.enumerated()),
                    id: \.element.id
                ) { index, preset in
                    VStack(alignment: .leading, spacing: 3) {
                        Text("\(index + 1). \(preset.name)").lineLimit(1)
                        HStack(spacing: 6) {
                            Text("모니터 \(preset.monitorCount)대")
                            if !preset.autoMatchSet.isEmpty {
                                Label(
                                    "자동 \(preset.autoMatchSet.count)",
                                    systemImage: "bolt.fill"
                                )
                            }
                        }
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    }
                    .tag(preset.id)
                }
            }
            HStack(spacing: 6) {
                Button(action: captureCurrentPreset) {
                    Image(systemName: "plus")
                }
                .help("현재 구성으로 새 프리셋")
                .disabled(store.settings.display.presets.count >= DisplayPresetDefaults.maximumPresetCount)

                Button(action: beginRename) {
                    Image(systemName: "pencil")
                }
                .help("이름 바꾸기")
                .disabled(selectedPreset == nil)

                Button(action: duplicateSelectedPreset) {
                    Image(systemName: "plus.square.on.square")
                }
                .help("복제")
                .disabled(
                    selectedPreset == nil
                        || store.settings.display.presets.count >= DisplayPresetDefaults.maximumPresetCount
                )

                Button(action: deleteSelectedPreset) {
                    Image(systemName: "trash")
                }
                .help("삭제")
                .disabled(selectedPreset == nil)

                Button {
                    moveSelectedPreset(offset: -1)
                } label: {
                    Image(systemName: "arrow.up")
                }
                .help("우선순위 올리기")
                .disabled(selectedPresetIndex == nil || selectedPresetIndex == 0)

                Button {
                    moveSelectedPreset(offset: 1)
                } label: {
                    Image(systemName: "arrow.down")
                }
                .help("우선순위 내리기")
                .disabled(
                    selectedPresetIndex == nil
                        || selectedPresetIndex == store.settings.display.presets.count - 1
                )
            }
            Text("최대 \(DisplayPresetDefaults.maximumPresetCount)개")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private func presetEditor(_ preset: DisplayPreset) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(preset.name).font(.title3.bold())
                Spacer()
                Button("계획 보기") { showPlan(for: preset) }
            }
            if let gridError = DisplayGrid.validationError(for: preset) {
                Label(
                    gridError.localizedDescription,
                    systemImage: "exclamationmark.triangle.fill"
                )
                .font(.callout)
                .foregroundStyle(.red)
            }
            ScrollView([.horizontal, .vertical]) {
                HStack(alignment: .top, spacing: 10) {
                    screenEditor(slot: .main, preset: preset)
                    if preset.secondary1 != nil {
                        screenEditor(slot: .secondary1, preset: preset)
                    } else {
                        addScreenCard(slot: .secondary1)
                    }
                    if preset.secondary2 != nil {
                        screenEditor(slot: .secondary2, preset: preset)
                    } else {
                        addScreenCard(slot: .secondary2)
                    }
                }
                .padding(.vertical, 2)
            }
        }
        .padding(.leading, 10)
    }

    private func screenEditor(slot: PresetScreenSlot, preset: DisplayPreset) -> some View {
        let screenValue = screen(slot, in: preset) ?? PresetScreen(monitors: [])
        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(slot.title).font(.headline)
                Spacer()
                if slot != .main {
                    Button(role: .destructive) {
                        removeScreen(slot)
                    } label: {
                        Image(systemName: "xmark.circle")
                    }
                    .buttonStyle(.plain)
                    .help("화면 칸 삭제")
                    .disabled(slot == .secondary1 && preset.secondary2 != nil)
                }
            }
            gridCellPicker(slot: slot, preset: preset)

            if screenValue.monitors.isEmpty {
                Text("모니터 없음")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 54)
            } else {
                ForEach(screenValue.monitors.indices, id: \.self) { index in
                    monitorRow(
                        screenValue.monitors[index],
                        index: index,
                        count: screenValue.monitors.count,
                        slot: slot
                    )
                }
            }

            Menu("모니터 추가") {
                if availableIdentities.isEmpty {
                    Text("추가할 모니터 없음")
                } else {
                    ForEach(availableIdentities, id: \.self) { identity in
                        Button(identity.name) { addMonitor(identity, to: slot) }
                    }
                }
            }
            .frame(maxWidth: .infinity)
        }
        .padding(10)
        .frame(width: 250, alignment: .topLeading)
        .background(.quaternary.opacity(0.35), in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(.separator))
    }

    private func gridCellPicker(
        slot: PresetScreenSlot,
        preset: DisplayPreset
    ) -> some View {
        let selectedCell = screen(slot, in: preset)?.cell ?? .center
        return VStack(alignment: .leading, spacing: 4) {
            Text("화면 칸 \(selectedCell.coordinateDescription)")
                .font(.caption)
                .foregroundStyle(.secondary)
            VStack(spacing: 3) {
                ForEach(-1...1, id: \.self) { y in
                    HStack(spacing: 3) {
                        ForEach(-1...1, id: \.self) { x in
                            let cell = GridCell(x: x, y: y)
                            let isCurrent = cell == selectedCell
                            Button {
                                updateScreen(slot) { $0.cell = cell }
                            } label: {
                                Text(gridCellLabel(cell, in: preset))
                                    .font(.caption2.bold())
                                    .frame(width: 30, height: 24)
                                    .foregroundStyle(isCurrent ? Color.white : Color.primary)
                                    .background(
                                        isCurrent
                                            ? Color.accentColor
                                            : Color.secondary.opacity(0.12),
                                        in: RoundedRectangle(cornerRadius: 4)
                                    )
                            }
                            .buttonStyle(.plain)
                            .disabled(slot == .main || !canAssign(cell, to: slot, in: preset))
                            .help(cell.coordinateDescription)
                        }
                    }
                }
            }
        }
    }

    private func gridCellLabel(_ cell: GridCell, in preset: DisplayPreset) -> String {
        if preset.main.cell == cell { return "주" }
        if preset.secondary1?.cell == cell { return "1" }
        if preset.secondary2?.cell == cell { return "2" }
        return "·"
    }

    private func canAssign(
        _ cell: GridCell,
        to slot: PresetScreenSlot,
        in preset: DisplayPreset
    ) -> Bool {
        guard slot != .main else { return cell == .center }
        var candidate = preset
        guard var candidateScreen = screen(slot, in: candidate) else { return false }
        candidateScreen.cell = cell
        setScreen(candidateScreen, slot: slot, in: &candidate)
        return DisplayGrid.validationError(for: candidate) == nil
    }

    private func addScreenCard(slot: PresetScreenSlot) -> some View {
        VStack {
            Spacer()
            Button {
                addScreen(slot)
            } label: {
                Label("\(slot.title) 추가", systemImage: "plus.rectangle")
            }
            .disabled(slot == .secondary2 && selectedPreset?.secondary1 == nil)
            Spacer()
        }
        .frame(width: 180, height: 130)
        .background(.quaternary.opacity(0.2), in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(.separator, style: StrokeStyle(dash: [4])))
    }

    private func monitorRow(
        _ presetMonitor: PresetMonitor,
        index: Int,
        count: Int,
        slot: PresetScreenSlot
    ) -> some View {
        let isConnected = matchedOnlineDisplay(for: presetMonitor.identity) != nil
        return VStack(alignment: .leading, spacing: 5) {
            HStack(alignment: .firstTextBaseline, spacing: 5) {
                Text(presetMonitor.identity.name)
                    .lineLimit(2)
                Spacer()
                if index == 0 {
                    Text("기준")
                        .font(.caption2.bold())
                        .padding(.horizontal, 5)
                        .padding(.vertical, 2)
                        .background(.blue.opacity(0.15), in: Capsule())
                }
            }
            HStack(spacing: 5) {
                Text(presetMonitor.identity.kind.displayName)
                Text(isConnected ? "연결됨" : "연결 안 됨")
                    .foregroundStyle(isConnected ? Color.green : Color.orange)
            }
            .font(.caption)
            Menu {
                let replacements = availableReplacementDisplays(
                    for: presetMonitor.identity
                )
                if replacements.isEmpty {
                    Text("지정할 연결 모니터 없음")
                } else {
                    ForEach(replacements) { display in
                        Button(display.name) {
                            replaceMonitorIdentity(
                                at: index,
                                in: slot,
                                with: display.identity
                            )
                        }
                    }
                }
            } label: {
                Label(
                    isConnected ? "다른 모니터로 바꾸기" : "다시 지정",
                    systemImage: isConnected ? "arrow.triangle.2.circlepath" : "exclamationmark.triangle.fill"
                )
                .foregroundStyle(isConnected ? Color.secondary : Color.orange)
            }
            .font(.caption)
            HStack(spacing: 4) {
                Toggle(
                    "자동",
                    isOn: Binding(
                        get: {
                            monitor(at: index, in: slot)?.autoMatch ?? presetMonitor.autoMatch
                        },
                        set: { enabled in
                            updateMonitor(at: index, in: slot) { $0.autoMatch = enabled }
                        }
                    )
                )
                .toggleStyle(.checkbox)
                Spacer()
                Button {
                    moveMonitorWithinScreen(slot: slot, from: index, offset: -1)
                } label: {
                    Image(systemName: "arrow.up")
                }
                .buttonStyle(.borderless)
                .disabled(index == 0)
                .help("위로")
                Button {
                    moveMonitorWithinScreen(slot: slot, from: index, offset: 1)
                } label: {
                    Image(systemName: "arrow.down")
                }
                .buttonStyle(.borderless)
                .disabled(index == count - 1)
                .help("아래로")
                Menu {
                    ForEach(destinationSlots(excluding: slot), id: \.self) { destination in
                        Button(destination.title) {
                            moveMonitor(from: slot, index: index, to: destination)
                        }
                    }
                } label: {
                    Image(systemName: "arrow.right.square")
                }
                .menuStyle(.borderlessButton)
                .help("다른 화면 칸으로 이동")
                Button(role: .destructive) {
                    removeMonitor(at: index, from: slot)
                } label: {
                    Image(systemName: "minus.circle")
                }
                .buttonStyle(.borderless)
                .disabled(slot == .main && count == 1)
                .help("제거")
            }
        }
        .padding(7)
        .background(.background, in: RoundedRectangle(cornerRadius: 7))
    }

    private var selectedPreset: DisplayPreset? {
        guard let selectedPresetID else { return nil }
        return store.settings.display.presets.first { $0.id == selectedPresetID }
    }

    private var selectedPresetIndex: Int? {
        guard let selectedPresetID else { return nil }
        return store.settings.display.presets.firstIndex { $0.id == selectedPresetID }
    }

    private var currentAutoMatchDescription: String? {
        let presets = store.settings.display.presets
        guard let matched = DisplayPlanner.presetForConnectedSet(
            connected: onlineDisplays,
            presets: presets
        ),
            let index = presets.firstIndex(where: { $0.id == matched.id })
        else {
            return nil
        }
        return "현재 연결 집합 일치: \(index + 1)순위 \(matched.name)"
    }

    private var knownIdentities: [DisplayIdentity] {
        var result: [DisplayIdentity] = []
        let candidates = onlineDisplays.map(\.identity)
            + store.settings.display.presets.flatMap { $0.screens.flatMap(\.monitors).map(\.identity) }
        for identity in candidates where !result.contains(where: {
            $0.representsSameDisplay(as: identity)
        }) {
            result.append(identity)
        }
        return result
    }

    private var availableIdentities: [DisplayIdentity] {
        guard let selectedPreset else { return [] }
        let used = selectedPreset.screens.flatMap(\.monitors).map(\.identity)
        return knownIdentities.filter { identity in
            !used.contains { $0.representsSameDisplay(as: identity) }
        }
    }

    private func availableReplacementDisplays(
        for currentIdentity: DisplayIdentity
    ) -> [OnlineDisplay] {
        guard let selectedPreset else { return [] }
        let usedByOtherRows = selectedPreset.screens
            .flatMap(\.monitors)
            .map(\.identity)
            .filter { !$0.representsSameDisplay(as: currentIdentity) }
        return onlineDisplays.filter { display in
            !display.identity.representsSameDisplay(as: currentIdentity)
                && !usedByOtherRows.contains {
                    $0.representsSameDisplay(as: display.identity)
                }
        }
    }

    private func refreshOnlineDisplays(showingErrors: Bool) {
        do {
            onlineDisplays = try OnlineDisplayProvider.current()
        } catch {
            NSLog("EduTools: failed to refresh online displays: %@", String(describing: error))
            if showingErrors {
                presentedMessage = DisplaySettingsMessage(
                    title: "모니터 목록을 읽을 수 없습니다",
                    detail: error.localizedDescription
                )
            }
        }
    }

    private func captureCurrentPreset() {
        guard store.settings.display.presets.count < DisplayPresetDefaults.maximumPresetCount else {
            return
        }
        do {
            let current = try OnlineDisplayProvider.current()
            onlineDisplays = current
            var capture = try DisplayPresetCapture.current(online: current)
            capture.preset.name = uniqueName(startingWith: capture.preset.name)
            let result = store.update { $0.display.presets.append(capture.preset) }
            if case .success = result {
                selectedPresetID = capture.preset.id
                if !capture.warnings.isEmpty {
                    presentedMessage = DisplaySettingsMessage(
                        title: "현재 구성 캡처",
                        detail: capture.warnings.joined(separator: "\n")
                    )
                }
            }
        } catch {
            NSLog("EduTools: failed to capture display preset: %@", String(describing: error))
            presentedMessage = DisplaySettingsMessage(
                title: "현재 구성을 캡처할 수 없습니다",
                detail: error.localizedDescription
            )
        }
    }

    private func beginRename() {
        guard let selectedPreset else { return }
        renameDraft = selectedPreset.name
        isRenaming = true
    }

    private func commitRename() {
        let trimmed = renameDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard isValidName(trimmed) else { return }
        updateSelectedPreset { $0.name = trimmed }
    }

    private func duplicateSelectedPreset() {
        guard var copy = selectedPreset,
              store.settings.display.presets.count < DisplayPresetDefaults.maximumPresetCount
        else {
            return
        }
        copy.id = UUID()
        copy.name = uniqueName(startingWith: "\(copy.name) 복사본")
        let result = store.update { settings in
            guard let sourceIndex = settings.display.presets.firstIndex(where: {
                $0.id == selectedPresetID
            }) else {
                return
            }
            settings.display.presets.insert(copy, at: sourceIndex + 1)
        }
        if case .success = result {
            selectedPresetID = copy.id
        }
    }

    private func deleteSelectedPreset() {
        guard let selectedPresetID,
              let index = store.settings.display.presets.firstIndex(where: {
                $0.id == selectedPresetID
              })
        else {
            return
        }
        let result = store.update { $0.display.presets.remove(at: index) }
        if case .success = result {
            let remaining = store.settings.display.presets
            self.selectedPresetID = remaining.indices.contains(index)
                ? remaining[index].id
                : remaining.last?.id
        }
    }

    private func moveSelectedPreset(offset: Int) {
        guard let source = selectedPresetIndex else { return }
        let destination = source + offset
        guard store.settings.display.presets.indices.contains(destination) else { return }
        store.update { settings in
            settings.display.presets.swapAt(source, destination)
        }
    }

    private func showPlan(for preset: DisplayPreset) {
        do {
            let current = try OnlineDisplayProvider.current()
            onlineDisplays = current
            let plan = DisplayPlanner.plan(preset: preset, online: current)
            presentedMessage = DisplaySettingsMessage(
                title: "화면 구성 계획",
                detail: plan.summary()
            )
        } catch {
            NSLog("EduTools: failed to calculate display plan: %@", String(describing: error))
            presentedMessage = DisplaySettingsMessage(
                title: "계획을 계산할 수 없습니다",
                detail: error.localizedDescription
            )
        }
    }

    private func addScreen(_ slot: PresetScreenSlot) {
        updateSelectedPreset { preset in
            switch slot {
            case .main:
                break
            case .secondary1:
                if preset.secondary1 == nil {
                    guard let cell = nextAvailableCell(in: preset) else {
                        NSLog("EduTools: no connected grid cell available for secondary screen 1")
                        return
                    }
                    preset.secondary1 = PresetScreen(monitors: [], cell: cell)
                }
            case .secondary2:
                if preset.secondary1 != nil, preset.secondary2 == nil {
                    guard let cell = nextAvailableCell(in: preset) else {
                        NSLog("EduTools: no connected grid cell available for secondary screen 2")
                        return
                    }
                    preset.secondary2 = PresetScreen(monitors: [], cell: cell)
                }
            }
        }
    }

    private func removeScreen(_ slot: PresetScreenSlot) {
        updateSelectedPreset { preset in
            switch slot {
            case .main:
                break
            case .secondary1:
                if preset.secondary2 == nil { preset.secondary1 = nil }
            case .secondary2:
                preset.secondary2 = nil
            }
        }
    }

    private func addMonitor(_ identity: DisplayIdentity, to slot: PresetScreenSlot) {
        updateScreen(slot) { $0.monitors.append(PresetMonitor(identity: identity)) }
    }

    private func replaceMonitorIdentity(
        at index: Int,
        in slot: PresetScreenSlot,
        with identity: DisplayIdentity
    ) {
        updateMonitor(at: index, in: slot) { $0.identity = identity }
    }

    private func removeMonitor(at index: Int, from slot: PresetScreenSlot) {
        updateScreen(slot) { screen in
            guard screen.monitors.indices.contains(index),
                  slot != .main || screen.monitors.count > 1
            else {
                return
            }
            screen.monitors.remove(at: index)
        }
    }

    private func moveMonitorWithinScreen(
        slot: PresetScreenSlot,
        from index: Int,
        offset: Int
    ) {
        updateScreen(slot) { screen in
            let destination = index + offset
            guard screen.monitors.indices.contains(index),
                  screen.monitors.indices.contains(destination)
            else {
                return
            }
            screen.monitors.swapAt(index, destination)
        }
    }

    private func moveMonitor(
        from source: PresetScreenSlot,
        index: Int,
        to destination: PresetScreenSlot
    ) {
        updateSelectedPreset { preset in
            guard var sourceScreen = screen(source, in: preset),
                  sourceScreen.monitors.indices.contains(index),
                  source != .main || sourceScreen.monitors.count > 1,
                  var destinationScreen = screen(destination, in: preset)
            else {
                return
            }
            let monitor = sourceScreen.monitors.remove(at: index)
            destinationScreen.monitors.append(monitor)
            setScreen(sourceScreen, slot: source, in: &preset)
            setScreen(destinationScreen, slot: destination, in: &preset)
        }
    }

    private func destinationSlots(excluding source: PresetScreenSlot) -> [PresetScreenSlot] {
        PresetScreenSlot.allCases.filter {
            $0 != source && screen($0, in: selectedPreset) != nil
        }
    }

    private func nextAvailableCell(in preset: DisplayPreset) -> GridCell? {
        let occupied = Set(preset.occupiedCells)
        return GridCell.placementOrder.first { candidate in
            !occupied.contains(candidate)
                && DisplayGrid.isConnected(preset.occupiedCells + [candidate])
        }
    }

    private func updateMonitor(
        at index: Int,
        in slot: PresetScreenSlot,
        change: (inout PresetMonitor) -> Void
    ) {
        updateScreen(slot) { screen in
            guard screen.monitors.indices.contains(index) else { return }
            change(&screen.monitors[index])
        }
    }

    private func updateScreen(
        _ slot: PresetScreenSlot,
        change: (inout PresetScreen) -> Void
    ) {
        updateSelectedPreset { preset in
            guard var value = screen(slot, in: preset) else { return }
            change(&value)
            setScreen(value, slot: slot, in: &preset)
        }
    }

    private func updateSelectedPreset(_ change: (inout DisplayPreset) -> Void) {
        guard let selectedPresetID else { return }
        store.update { settings in
            guard let index = settings.display.presets.firstIndex(where: {
                $0.id == selectedPresetID
            }) else {
                return
            }
            change(&settings.display.presets[index])
        }
    }

    private func screen(
        _ slot: PresetScreenSlot,
        in preset: DisplayPreset?
    ) -> PresetScreen? {
        guard let preset else { return nil }
        switch slot {
        case .main: return preset.main
        case .secondary1: return preset.secondary1
        case .secondary2: return preset.secondary2
        }
    }

    private func setScreen(
        _ screen: PresetScreen,
        slot: PresetScreenSlot,
        in preset: inout DisplayPreset
    ) {
        switch slot {
        case .main: preset.main = screen
        case .secondary1: preset.secondary1 = screen
        case .secondary2: preset.secondary2 = screen
        }
    }

    private func monitor(at index: Int, in slot: PresetScreenSlot) -> PresetMonitor? {
        guard let value = screen(slot, in: selectedPreset),
              value.monitors.indices.contains(index)
        else {
            return nil
        }
        return value.monitors[index]
    }

    private func matchedOnlineDisplay(for identity: DisplayIdentity) -> OnlineDisplay? {
        let matches = DisplayMatcher.matches(
            monitors: [PresetMonitor(identity: identity)],
            online: onlineDisplays
        )
        guard let index = matches.first?.onlineIndex else { return nil }
        return onlineDisplays[index]
    }

    private func uniqueName(startingWith requestedName: String) -> String {
        let existing = Set(store.settings.display.presets.map(\.name))
        let initial = limitedName(requestedName)
        guard existing.contains(initial) else { return initial }
        var suffix = 2
        while true {
            let suffixText = " \(suffix)"
            let baseLimit = DisplayPresetDefaults.maximumNameLength - suffixText.count
            let base = String(requestedName.prefix(max(baseLimit, 1)))
            let candidate = base + suffixText
            if !existing.contains(candidate) { return candidate }
            suffix += 1
        }
    }

    private func limitedName(_ name: String) -> String {
        String(name.prefix(DisplayPresetDefaults.maximumNameLength))
    }

    private func isValidName(_ name: String) -> Bool {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        return !trimmed.isEmpty && name.count <= DisplayPresetDefaults.maximumNameLength
    }
}
