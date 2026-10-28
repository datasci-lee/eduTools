import Foundation

enum DisplayPresetDefaults {
    static let maximumPresetCount = 8
    static let maximumNameLength = 40
    static let connectionStabilityInterval: TimeInterval = 5
    static let automaticApplyCooldown: TimeInterval = 30
    static let manualConfirmationInterval: TimeInterval = 15
}

enum DisplayKind: String, Codable, CaseIterable, Hashable {
    case builtIn
    case external
    case virtual

    var displayName: String {
        switch self {
        case .builtIn: return "내장"
        case .external: return "외장"
        case .virtual: return "가상"
        }
    }
}

struct GridCell: Codable, Equatable, Hashable {
    var x: Int
    var y: Int

    static let center = GridCell(x: 0, y: 0)
    static let all: [GridCell] = (-1...1).flatMap { y in
        (-1...1).map { x in GridCell(x: x, y: y) }
    }
    static let placementOrder: [GridCell] = [
        GridCell(x: 1, y: 0),
        GridCell(x: -1, y: 0),
        GridCell(x: 0, y: -1),
        GridCell(x: 0, y: 1),
        GridCell(x: -1, y: -1),
        GridCell(x: 1, y: -1),
        GridCell(x: -1, y: 1),
        GridCell(x: 1, y: 1),
    ]

    var isInBounds: Bool {
        (-1...1).contains(x) && (-1...1).contains(y)
    }

    var coordinateDescription: String {
        "(\(x),\(y))"
    }

    func isOrthogonallyAdjacent(to other: GridCell) -> Bool {
        abs(x - other.x) + abs(y - other.y) == 1
    }
}

struct DisplayPoint: Codable, Equatable, Hashable {
    var x: Double
    var y: Double

    static let zero = DisplayPoint(x: 0, y: 0)
}

struct DisplaySize: Codable, Equatable, Hashable {
    var width: Double
    var height: Double
}

struct DisplayIdentity: Codable, Equatable, Hashable {
    var uuid: String
    var vendor: UInt32
    var model: UInt32
    var serial: UInt32
    var name: String
    var kind: DisplayKind

    fileprivate func matchLevel(with other: DisplayIdentity) -> DisplayMatchLevel? {
        if !uuid.isEmpty,
           !other.uuid.isEmpty,
           uuid.caseInsensitiveCompare(other.uuid) == .orderedSame
        {
            return .uuid
        }
        if serial != 0,
           other.serial != 0,
           vendor == other.vendor,
           model == other.model,
           serial == other.serial
        {
            return .hardwareSerial
        }
        if kind == .virtual,
           other.kind == .virtual,
           vendor == other.vendor,
           model == other.model,
           name == other.name
        {
            return .virtualName
        }
        return nil
    }

    func representsSameDisplay(as other: DisplayIdentity) -> Bool {
        self == other || matchLevel(with: other) != nil
    }
}

struct PresetMonitor: Codable, Equatable, Hashable {
    var identity: DisplayIdentity
    var autoMatch: Bool

    init(identity: DisplayIdentity, autoMatch: Bool = false) {
        self.identity = identity
        self.autoMatch = autoMatch
    }
}

struct PresetScreen: Codable, Equatable, Hashable {
    var monitors: [PresetMonitor]
    var cell: GridCell

    init(monitors: [PresetMonitor], cell: GridCell = .center) {
        self.monitors = monitors
        self.cell = cell
    }
}

struct DisplayPreset: Codable, Equatable, Identifiable {
    var id: UUID
    var name: String
    var main: PresetScreen
    var secondary1: PresetScreen?
    var secondary2: PresetScreen?

    init(
        id: UUID = UUID(),
        name: String,
        main: PresetScreen,
        secondary1: PresetScreen? = nil,
        secondary2: PresetScreen? = nil
    ) {
        self.id = id
        self.name = name
        self.main = main
        self.secondary1 = secondary1
        self.secondary2 = secondary2
    }

    var screens: [PresetScreen] {
        [main, secondary1, secondary2].compactMap { $0 }
    }

    var monitorCount: Int {
        screens.reduce(0) { $0 + $1.monitors.count }
    }

    var autoMatchSet: Set<DisplayIdentity> {
        Set(screens.flatMap(\.monitors).filter(\.autoMatch).map(\.identity))
    }

    var occupiedCells: [GridCell] {
        screens.map(\.cell)
    }

    func matchesCurrent(online: [OnlineDisplay]) -> Bool {
        DisplayPlanner.matchesCurrent(preset: self, online: online)
    }

    func matchesAutoSet(online: [OnlineDisplay]) -> Bool {
        let monitors = screens.flatMap(\.monitors).filter(\.autoMatch)
        guard !monitors.isEmpty, monitors.count == online.count else { return false }
        return DisplayMatcher.matches(monitors: monitors, online: online).count == monitors.count
    }

}

struct DisplaySettings: Codable, Equatable {
    var autoSwitchEnabled: Bool
    var presets: [DisplayPreset]

    static let `default` = DisplaySettings()

    init(autoSwitchEnabled: Bool = false, presets: [DisplayPreset] = []) {
        self.autoSwitchEnabled = autoSwitchEnabled
        self.presets = presets
    }
}

enum DisplayPresetValidationError: LocalizedError, Equatable {
    case tooManyPresets
    case duplicatePresetID
    case invalidName
    case emptyMainScreen
    case duplicateMonitor(String)
    case secondaryTwoRequiresSecondaryOne
    case mainScreenMustUseCenter
    case invalidGridCell
    case duplicateGridCell
    case disconnectedGrid

    var errorDescription: String? {
        switch self {
        case .tooManyPresets:
            return "화면 구성 프리셋은 최대 \(DisplayPresetDefaults.maximumPresetCount)개까지 저장할 수 있습니다"
        case .duplicatePresetID:
            return "화면 구성 프리셋 식별자가 중복됩니다"
        case .invalidName:
            return "화면 구성 프리셋 이름은 1~\(DisplayPresetDefaults.maximumNameLength)자여야 합니다"
        case .emptyMainScreen:
            return "주 화면에는 모니터가 하나 이상 있어야 합니다"
        case let .duplicateMonitor(name):
            return "한 프리셋에서 같은 모니터를 두 번 사용할 수 없습니다: \(name)"
        case .secondaryTwoRequiresSecondaryOne:
            return "보조화면2를 사용하려면 보조화면1이 있어야 합니다"
        case .mainScreenMustUseCenter:
            return "주 화면은 가운데 칸 (0,0)에 있어야 합니다"
        case .invalidGridCell:
            return "화면 칸은 3×3 격자 안에 있어야 합니다"
        case .duplicateGridCell:
            return "두 화면을 같은 격자 칸에 둘 수 없습니다"
        case .disconnectedGrid:
            return "모든 화면은 위·아래·왼쪽·오른쪽으로 이어져 주 화면에 연결되어야 합니다"
        }
    }
}

enum DisplayGrid {
    static func validationError(for preset: DisplayPreset) -> DisplayPresetValidationError? {
        let cells = preset.occupiedCells
        guard preset.main.cell == .center else { return .mainScreenMustUseCenter }
        guard cells.allSatisfy(\.isInBounds) else { return .invalidGridCell }
        guard Set(cells).count == cells.count else { return .duplicateGridCell }
        guard isConnected(cells) else { return .disconnectedGrid }
        return nil
    }

    static func isConnected(_ cells: [GridCell]) -> Bool {
        let occupied = Set(cells)
        guard occupied.contains(.center) else { return false }
        var reached: Set<GridCell> = [.center]
        var pending: [GridCell] = [.center]
        while let cell = pending.popLast() {
            for candidate in occupied where !reached.contains(candidate)
                && cell.isOrthogonallyAdjacent(to: candidate)
            {
                reached.insert(candidate)
                pending.append(candidate)
            }
        }
        return reached == occupied
    }
}

enum DisplayPresetValidator {
    static func validate(_ settings: DisplaySettings) throws {
        guard settings.presets.count <= DisplayPresetDefaults.maximumPresetCount else {
            throw DisplayPresetValidationError.tooManyPresets
        }
        guard Set(settings.presets.map(\.id)).count == settings.presets.count else {
            throw DisplayPresetValidationError.duplicatePresetID
        }
        for preset in settings.presets {
            let trimmedName = preset.name.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmedName.isEmpty,
                  preset.name.count <= DisplayPresetDefaults.maximumNameLength
            else {
                throw DisplayPresetValidationError.invalidName
            }
            guard !preset.main.monitors.isEmpty else {
                throw DisplayPresetValidationError.emptyMainScreen
            }
            guard preset.secondary2 == nil || preset.secondary1 != nil else {
                throw DisplayPresetValidationError.secondaryTwoRequiresSecondaryOne
            }
            if let gridError = DisplayGrid.validationError(for: preset) {
                throw gridError
            }
            let identities = preset.screens.flatMap(\.monitors).map(\.identity)
            for leftIndex in identities.indices {
                for rightIndex in identities.indices where rightIndex > leftIndex {
                    if identities[leftIndex].representsSameDisplay(as: identities[rightIndex]) {
                        throw DisplayPresetValidationError.duplicateMonitor(
                            identities[rightIndex].name
                        )
                    }
                }
            }
        }
    }
}

struct OnlineDisplay: Equatable, Identifiable {
    var id: UInt32
    var uuid: String
    var vendor: UInt32
    var model: UInt32
    var serial: UInt32
    var name: String
    var kind: DisplayKind
    var size: DisplaySize
    var origin: DisplayPoint
    var currentMirrorMasterID: UInt32?

    var identity: DisplayIdentity {
        DisplayIdentity(
            uuid: uuid,
            vendor: vendor,
            model: model,
            serial: serial,
            name: name,
            kind: kind
        )
    }
}

fileprivate enum DisplayMatchLevel: Int, CaseIterable {
    case uuid
    case hardwareSerial
    case virtualName
}

struct DisplayMatch: Equatable {
    var presetIndex: Int
    var onlineIndex: Int
}

enum DisplayMatcher {
    static func matches(monitors: [PresetMonitor], online: [OnlineDisplay]) -> [DisplayMatch] {
        match(
            leftCount: monitors.count,
            rightCount: online.count,
            level: { monitors[$0].identity.matchLevel(with: online[$1].identity) }
        )
    }

    private static func match(
        leftCount: Int,
        rightCount: Int,
        level: (Int, Int) -> DisplayMatchLevel?
    ) -> [DisplayMatch] {
        var usedLeft: Set<Int> = []
        var usedRight: Set<Int> = []
        var result: [DisplayMatch] = []
        for matchLevel in DisplayMatchLevel.allCases {
            for leftIndex in 0..<leftCount where !usedLeft.contains(leftIndex) {
                guard let rightIndex = (0..<rightCount).first(where: {
                    !usedRight.contains($0) && level(leftIndex, $0) == matchLevel
                }) else {
                    continue
                }
                usedLeft.insert(leftIndex)
                usedRight.insert(rightIndex)
                result.append(DisplayMatch(presetIndex: leftIndex, onlineIndex: rightIndex))
            }
        }
        return result.sorted { $0.presetIndex < $1.presetIndex }
    }
}

enum DisplayScreenRole: Int, Equatable {
    case main
    case secondary1
    case secondary2

    var displayName: String {
        switch self {
        case .main: return "주 화면"
        case .secondary1: return "보조화면1"
        case .secondary2: return "보조화면2"
        }
    }
}

struct DisplayMirrorPlan: Equatable {
    var slaveID: UInt32
    var masterID: UInt32
}

struct DisplayPlannedScreen: Equatable {
    var role: DisplayScreenRole
    var masterID: UInt32
    var masterName: String
    var mirroredDisplayIDs: [UInt32]
    var mirroredNames: [String]
    var cell: GridCell
    var origin: DisplayPoint
    var size: DisplaySize
}

struct DisplayOutsidePlacement: Equatable {
    var displayID: UInt32
    var name: String
    var origin: DisplayPoint
}

struct DisplayPlan: Equatable {
    var mainDisplayID: UInt32?
    var originsByMaster: [UInt32: DisplayPoint]
    var mirrors: [DisplayMirrorPlan]
    var unmirrorDisplayIDs: [UInt32]
    var outsidePlacements: [DisplayOutsidePlacement]
    var missingPresetMonitors: [DisplayIdentity]
    var screens: [DisplayPlannedScreen]
    var layoutIssues: [String]

    var isApplicable: Bool {
        mainDisplayID != nil && layoutIssues.isEmpty
    }

    func summary() -> String {
        var parts: [String] = []
        if !layoutIssues.isEmpty {
            parts.append("적용 불가: " + layoutIssues.joined(separator: " · "))
        }
        if let anchor = screens.first, anchor.role != .main {
            parts.append(
                "주 화면 모니터 없음 → \(anchor.role.displayName) 이 주 디스플레이"
            )
        }
        parts.append(contentsOf: screens.map { screen in
            var value = "\(screen.role.displayName) = \(screen.masterName)"
            if !screen.mirroredNames.isEmpty {
                value += "(미러: \(screen.mirroredNames.joined(separator: ", ")))"
            }
            value += " \(screen.cell.coordinateDescription), 원점 \(pointDescription(screen.origin))"
            return value
        })
        if screens.count > 1 {
            let ordered = screens.sorted {
                $0.cell.y == $1.cell.y ? $0.cell.x < $1.cell.x : $0.cell.y < $1.cell.y
            }
            parts.append(
                "격자 순서: " + ordered.map {
                    "\($0.role.displayName)\($0.cell.coordinateDescription)"
                }.joined(separator: " – ")
            )
        }
        if !outsidePlacements.isEmpty {
            parts.append(
                "프리셋 밖 = \(outsidePlacements.map(\.name).joined(separator: ", ")) (배치 묶음 오른쪽에 확장)"
            )
        }
        if !missingPresetMonitors.isEmpty {
            parts.append(
                "연결 안 됨: \(missingPresetMonitors.map(\.name).joined(separator: ", "))"
            )
        }
        if !unmirrorDisplayIDs.isEmpty {
            parts.append("미러 해제: \(unmirrorDisplayIDs.map(String.init).joined(separator: ", "))")
        }
        return parts.isEmpty ? "계산할 수 있는 연결된 모니터가 없습니다." : parts.joined(separator: " · ")
    }

    private func pointDescription(_ point: DisplayPoint) -> String {
        "(\(numberDescription(point.x)),\(numberDescription(point.y)))"
    }

    private func numberDescription(_ value: Double) -> String {
        value.rounded() == value ? String(Int(value)) : String(format: "%.2f", value)
    }
}

struct DisplayGridMetric {
    var cell: GridCell
    var size: DisplaySize
}

enum DisplayGridLayout {
    static func origins(for metrics: [DisplayGridMetric]) -> [GridCell: DisplayPoint] {
        guard let anchor = metrics.first else { return [:] }
        precondition(
            Set(metrics.map(\.cell)).count == metrics.count,
            "Display layout requires unique grid cells"
        )

        var result: [GridCell: DisplayPoint] = [anchor.cell: .zero]
        var placedIndices = [metrics.startIndex]
        var placed = Set(placedIndices)
        var pending = placedIndices
        var pendingIndex = 0

        while placed.count < metrics.count {
            while pendingIndex < pending.count {
                let parentIndex = pending[pendingIndex]
                pendingIndex += 1
                let parent = metrics[parentIndex]
                guard let parentOrigin = result[parent.cell] else {
                    preconditionFailure("Placed display is missing its origin")
                }
                for candidateIndex in metrics.indices where !placed.contains(candidateIndex) {
                    let candidate = metrics[candidateIndex]
                    guard candidate.cell.isOrthogonallyAdjacent(to: parent.cell) else {
                        continue
                    }
                    result[candidate.cell] = adjacentOrigin(
                        for: candidate,
                        nextTo: parent,
                        parentOrigin: parentOrigin
                    )
                    placed.insert(candidateIndex)
                    placedIndices.append(candidateIndex)
                    pending.append(candidateIndex)
                }
            }

            guard placed.count < metrics.count,
                  let detachedIndex = metrics.indices.first(where: { !placed.contains($0) })
            else {
                continue
            }
            result[metrics[detachedIndex].cell] = detachedOrigin(
                for: metrics[detachedIndex],
                anchorCell: anchor.cell,
                metrics: metrics,
                placedIndices: placedIndices,
                origins: result
            )
            placed.insert(detachedIndex)
            placedIndices.append(detachedIndex)
            pending.append(detachedIndex)
        }
        return result
    }

    private static func adjacentOrigin(
        for metric: DisplayGridMetric,
        nextTo parent: DisplayGridMetric,
        parentOrigin: DisplayPoint
    ) -> DisplayPoint {
        switch (metric.cell.x - parent.cell.x, metric.cell.y - parent.cell.y) {
        case (1, 0):
            return DisplayPoint(
                x: parentOrigin.x + parent.size.width,
                y: parentOrigin.y
            )
        case (-1, 0):
            return DisplayPoint(
                x: parentOrigin.x - metric.size.width,
                y: parentOrigin.y
            )
        case (0, 1):
            return DisplayPoint(
                x: parentOrigin.x,
                y: parentOrigin.y + parent.size.height
            )
        case (0, -1):
            return DisplayPoint(
                x: parentOrigin.x,
                y: parentOrigin.y - metric.size.height
            )
        default:
            preconditionFailure("Adjacent display cells must share an edge")
        }
    }

    private static func detachedOrigin(
        for metric: DisplayGridMetric,
        anchorCell: GridCell,
        metrics: [DisplayGridMetric],
        placedIndices: [Int],
        origins: [GridCell: DisplayPoint]
    ) -> DisplayPoint {
        let deltaX = metric.cell.x - anchorCell.x
        let deltaY = metric.cell.y - anchorCell.y
        let isHorizontal = deltaX != 0 && abs(deltaX) >= abs(deltaY)

        if isHorizontal {
            if deltaX > 0 {
                let edge = placedIndices.map {
                    origin(for: metrics[$0], in: origins).x + metrics[$0].size.width
                }.max() ?? 0
                let support = placedIndices.first {
                    let placedMetric = metrics[$0]
                    return origin(for: placedMetric, in: origins).x
                        + placedMetric.size.width == edge
                } ?? placedIndices[0]
                return DisplayPoint(
                    x: edge,
                    y: origin(for: metrics[support], in: origins).y
                )
            }
            let edge = placedIndices.map {
                origin(for: metrics[$0], in: origins).x
            }.min() ?? 0
            let support = placedIndices.first {
                origin(for: metrics[$0], in: origins).x == edge
            } ?? placedIndices[0]
            return DisplayPoint(
                x: edge - metric.size.width,
                y: origin(for: metrics[support], in: origins).y
            )
        }

        if deltaY > 0 {
            let edge = placedIndices.map {
                origin(for: metrics[$0], in: origins).y + metrics[$0].size.height
            }.max() ?? 0
            let support = placedIndices.first {
                let placedMetric = metrics[$0]
                return origin(for: placedMetric, in: origins).y
                    + placedMetric.size.height == edge
            } ?? placedIndices[0]
            return DisplayPoint(
                x: origin(for: metrics[support], in: origins).x,
                y: edge
            )
        }
        let edge = placedIndices.map {
            origin(for: metrics[$0], in: origins).y
        }.min() ?? 0
        let support = placedIndices.first {
            origin(for: metrics[$0], in: origins).y == edge
        } ?? placedIndices[0]
        return DisplayPoint(
            x: origin(for: metrics[support], in: origins).x,
            y: edge - metric.size.height
        )
    }

    private static func origin(
        for metric: DisplayGridMetric,
        in origins: [GridCell: DisplayPoint]
    ) -> DisplayPoint {
        guard let origin = origins[metric.cell] else {
            preconditionFailure("Placed display is missing its origin")
        }
        return origin
    }
}

enum DisplayPlanner {
    private struct MonitorSlot {
        var role: DisplayScreenRole
        var monitor: PresetMonitor
        var globalIndex: Int
    }

    private struct ResolvedScreen {
        var role: DisplayScreenRole
        var screen: PresetScreen
        var master: OnlineDisplay
        var slaves: [OnlineDisplay]
    }

    static func plan(preset: DisplayPreset, online: [OnlineDisplay]) -> DisplayPlan {
        var roleScreens: [(DisplayScreenRole, PresetScreen)] = [(.main, preset.main)]
        if let secondary1 = preset.secondary1 {
            roleScreens.append((.secondary1, secondary1))
        }
        if let secondary2 = preset.secondary2 {
            roleScreens.append((.secondary2, secondary2))
        }

        var slots: [MonitorSlot] = []
        for (role, screen) in roleScreens {
            for monitor in screen.monitors {
                slots.append(
                    MonitorSlot(
                        role: role,
                        monitor: monitor,
                        globalIndex: slots.count
                    )
                )
            }
        }
        let matches = DisplayMatcher.matches(monitors: slots.map(\.monitor), online: online)
        let onlineIndexBySlot = Dictionary(uniqueKeysWithValues: matches.map {
            ($0.presetIndex, $0.onlineIndex)
        })
        let matchedOnlineIndices = Set(matches.map(\.onlineIndex))

        let resolvedScreens = roleScreens.compactMap { role, screen -> ResolvedScreen? in
            let screenSlots = slots.filter { $0.role == role }
            let connected = screenSlots.compactMap { slot -> OnlineDisplay? in
                guard let onlineIndex = onlineIndexBySlot[slot.globalIndex] else { return nil }
                return online[onlineIndex]
            }
            guard let master = connected.first else { return nil }
            return ResolvedScreen(
                role: role,
                screen: screen,
                master: master,
                slaves: Array(connected.dropFirst())
            )
        }
        let gridOrigins = DisplayGridLayout.origins(for: resolvedScreens.map {
            DisplayGridMetric(cell: $0.screen.cell, size: $0.master.size)
        })
        var origins: [UInt32: DisplayPoint] = [:]
        var mirrors: [DisplayMirrorPlan] = []
        var plannedScreens: [DisplayPlannedScreen] = []
        for resolved in resolvedScreens {
            guard let origin = gridOrigins[resolved.screen.cell] else {
                preconditionFailure("Grid origin missing for a resolved preset screen")
            }
            let master = resolved.master
            origins[master.id] = origin
            mirrors.append(contentsOf: resolved.slaves.map {
                DisplayMirrorPlan(slaveID: $0.id, masterID: master.id)
            })
            plannedScreens.append(
                DisplayPlannedScreen(
                    role: resolved.role,
                    masterID: master.id,
                    masterName: master.name,
                    mirroredDisplayIDs: resolved.slaves.map(\.id),
                    mirroredNames: resolved.slaves.map(\.name),
                    cell: resolved.screen.cell,
                    origin: origin,
                    size: master.size
                )
            )
        }

        var outside = online.indices.filter { !matchedOnlineIndices.contains($0) }
        if plannedScreens.isEmpty {
            outside.sort { left, right in
                let leftIsCurrentMain = isZero(online[left].origin)
                    && online[left].currentMirrorMasterID == nil
                let rightIsCurrentMain = isZero(online[right].origin)
                    && online[right].currentMirrorMasterID == nil
                if leftIsCurrentMain != rightIsCurrentMain {
                    return leftIsCurrentMain
                }
                return left < right
            }
        }
        var outsidePlacements: [DisplayOutsidePlacement] = []
        var outsideX = plannedScreens.map { $0.origin.x + $0.size.width }.max() ?? 0
        let outsideY = plannedScreens.first(where: {
            $0.origin.x + $0.size.width == outsideX
        })?.origin.y ?? 0
        for onlineIndex in outside {
            let display = online[onlineIndex]
            let origin = DisplayPoint(x: outsideX, y: outsideY)
            origins[display.id] = origin
            outsidePlacements.append(
                DisplayOutsidePlacement(displayID: display.id, name: display.name, origin: origin)
            )
            outsideX += display.size.width
        }

        let desiredMirrorMaster = Dictionary(uniqueKeysWithValues: mirrors.map {
            ($0.slaveID, $0.masterID)
        })
        let unmirror = online.compactMap { display -> UInt32? in
            guard let currentMaster = display.currentMirrorMasterID,
                  desiredMirrorMaster[display.id] != currentMaster
            else {
                return nil
            }
            return display.id
        }
        let missing = slots.compactMap { slot in
            onlineIndexBySlot[slot.globalIndex] == nil ? slot.monitor.identity : nil
        }
        let layoutIssues = overlappingLayoutIssues(in: plannedScreens)
        return DisplayPlan(
            mainDisplayID: plannedScreens.first?.masterID ?? outsidePlacements.first?.displayID,
            originsByMaster: origins,
            mirrors: mirrors,
            unmirrorDisplayIDs: unmirror,
            outsidePlacements: outsidePlacements,
            missingPresetMonitors: missing,
            screens: plannedScreens,
            layoutIssues: layoutIssues
        )
    }

    private static func overlappingLayoutIssues(
        in screens: [DisplayPlannedScreen]
    ) -> [String] {
        var issues: [String] = []
        for leftIndex in screens.indices {
            for rightIndex in screens.indices where rightIndex > leftIndex {
                let left = screens[leftIndex]
                let right = screens[rightIndex]
                guard rectanglesOverlap(left, right) else { continue }
                issues.append(
                    "화면이 겹칩니다(\(left.role.displayName) ↔ \(right.role.displayName))"
                )
            }
        }
        return issues
    }

    private static func rectanglesOverlap(
        _ left: DisplayPlannedScreen,
        _ right: DisplayPlannedScreen
    ) -> Bool {
        left.origin.x < right.origin.x + right.size.width
            && right.origin.x < left.origin.x + left.size.width
            && left.origin.y < right.origin.y + right.size.height
            && right.origin.y < left.origin.y + left.size.height
    }

    static func presetForConnectedSet(
        connected: [OnlineDisplay],
        presets: [DisplayPreset]
    ) -> DisplayPreset? {
        presets.first { $0.matchesAutoSet(online: connected) }
    }

    static func matchesCurrent(preset: DisplayPreset, online: [OnlineDisplay]) -> Bool {
        let plan = plan(preset: preset, online: online)
        let currentMain = online.first {
            $0.currentMirrorMasterID == nil && isZero($0.origin)
        }?.id
        guard currentMain == plan.mainDisplayID else { return false }
        let desiredMirrorMaster = Dictionary(uniqueKeysWithValues: plan.mirrors.map {
            ($0.slaveID, $0.masterID)
        })
        for display in online {
            if display.currentMirrorMasterID != desiredMirrorMaster[display.id] {
                return false
            }
            if desiredMirrorMaster[display.id] == nil,
               let origin = plan.originsByMaster[display.id],
               !approximatelyEqual(origin, display.origin)
            {
                return false
            }
        }
        return true
    }

    private static func approximatelyEqual(_ left: DisplayPoint, _ right: DisplayPoint) -> Bool {
        abs(left.x - right.x) < 0.5 && abs(left.y - right.y) < 0.5
    }

    private static func isZero(_ point: DisplayPoint) -> Bool {
        approximatelyEqual(point, .zero)
    }
}

struct DisplayPresetCaptureResult: Equatable {
    var preset: DisplayPreset
    var warnings: [String]
}

enum DisplayPresetCaptureError: LocalizedError {
    case noOnlineDisplays

    var errorDescription: String? {
        switch self {
        case .noOnlineDisplays:
            return "연결된 모니터가 없어 현재 구성을 캡처할 수 없습니다."
        }
    }
}

enum DisplayPresetCapture {
    private struct MirrorGroup {
        var key: UInt32
        var firstOnlineIndex: Int
        var displays: [OnlineDisplay]

        var master: OnlineDisplay {
            displays.first(where: { $0.id == key }) ?? displays[0]
        }
    }

    static func current(
        online: [OnlineDisplay],
        name: String = "현재 구성"
    ) throws -> DisplayPresetCaptureResult {
        guard !online.isEmpty else { throw DisplayPresetCaptureError.noOnlineDisplays }
        var grouped: [UInt32: [(Int, OnlineDisplay)]] = [:]
        for (index, display) in online.enumerated() {
            grouped[display.currentMirrorMasterID ?? display.id, default: []].append((index, display))
        }
        var groups = grouped.map { key, indexedDisplays -> MirrorGroup in
            let sorted = indexedDisplays.sorted { left, right in
                if left.1.id == key { return true }
                if right.1.id == key { return false }
                return left.0 < right.0
            }
            return MirrorGroup(
                key: key,
                firstOnlineIndex: indexedDisplays.map { $0.0 }.min() ?? 0,
                displays: sorted.map { $0.1 }
            )
        }
        var warnings: [String] = []
        let mainGroupIndex = groups.firstIndex { group in
            group.displays.contains {
                $0.currentMirrorMasterID == nil && DisplayPlanner.matchesOriginZero($0.origin)
            }
        } ?? {
            warnings.append("주 화면을 원점에서 찾지 못해 온라인 목록의 첫 화면을 주 화면으로 사용했습니다.")
            return groups.indices.min(by: {
                groups[$0].firstOnlineIndex < groups[$1].firstOnlineIndex
            }) ?? 0
        }()
        let mainGroup = groups.remove(at: mainGroupIndex)
        groups.sort { left, right in
            let leftDistance = squaredDistance(left.master.origin, mainGroup.master.origin)
            let rightDistance = squaredDistance(right.master.origin, mainGroup.master.origin)
            if leftDistance != rightDistance { return leftDistance < rightDistance }
            return left.firstOnlineIndex < right.firstOnlineIndex
        }
        let ordered = [mainGroup] + groups
        if ordered.count > 3 {
            warnings.append(
                "화면이 \(ordered.count)개여서 원점 거리순으로 가까운 3개 화면만 담았습니다."
            )
        }
        let captured = Array(ordered.prefix(3))
        let assignment = closestConnectedCells(for: captured)
        if !assignment.isExact {
            let coordinates = assignment.cells.map(\.coordinateDescription).joined(separator: " · ")
            warnings.append(
                "현재 원점을 연결된 3×3 격자의 가장 가까운 칸으로 근사했습니다: \(coordinates)"
            )
        }
        let screens = captured.enumerated().map { index, group -> PresetScreen in
            return PresetScreen(
                monitors: group.displays.map {
                    PresetMonitor(identity: $0.identity, autoMatch: false)
                },
                cell: assignment.cells[index]
            )
        }
        return DisplayPresetCaptureResult(
            preset: DisplayPreset(
                name: name,
                main: screens[0],
                secondary1: screens.indices.contains(1) ? screens[1] : nil,
                secondary2: screens.indices.contains(2) ? screens[2] : nil
            ),
            warnings: warnings
        )
    }

    private struct CellAssignment {
        var cells: [GridCell]
        var score: Double
        var isExact: Bool
    }

    private static func closestConnectedCells(for groups: [MirrorGroup]) -> CellAssignment {
        guard groups.count > 1 else {
            return CellAssignment(cells: [.center], score: 0, isExact: true)
        }
        let mainOrigin = groups[0].master.origin
        let actualOrigins = groups.map {
            DisplayPoint(
                x: $0.master.origin.x - mainOrigin.x,
                y: $0.master.origin.y - mainOrigin.y
            )
        }
        var best: CellAssignment?

        func consider(_ secondaryCells: [GridCell]) {
            let cells = [.center] + secondaryCells
            guard DisplayGrid.isConnected(cells) else { return }
            let metrics = zip(cells, groups).map { cell, group in
                DisplayGridMetric(cell: cell, size: group.master.size)
            }
            let origins = DisplayGridLayout.origins(for: metrics)
            var score = 0.0
            var isExact = true
            for index in cells.indices {
                guard let planned = origins[cells[index]] else {
                    preconditionFailure("Grid origin missing while capturing a preset")
                }
                let actual = actualOrigins[index]
                score += squaredDistance(planned, actual)
                if abs(planned.x - actual.x) >= 0.5 || abs(planned.y - actual.y) >= 0.5 {
                    isExact = false
                }
            }
            if best == nil || score < best!.score {
                best = CellAssignment(cells: cells, score: score, isExact: isExact)
            }
        }

        func assign(_ cells: [GridCell]) {
            if cells.count == groups.count - 1 {
                consider(cells)
                return
            }
            for candidate in GridCell.placementOrder where !cells.contains(candidate) {
                assign(cells + [candidate])
            }
        }

        assign([])
        guard let best else {
            preconditionFailure("No connected grid assignment for at most three screens")
        }
        return best
    }

    private static func squaredDistance(_ left: DisplayPoint, _ right: DisplayPoint) -> Double {
        let deltaX = left.x - right.x
        let deltaY = left.y - right.y
        return deltaX * deltaX + deltaY * deltaY
    }
}

private extension DisplayPlanner {
    static func matchesOriginZero(_ point: DisplayPoint) -> Bool {
        abs(point.x) < 0.5 && abs(point.y) < 0.5
    }
}
