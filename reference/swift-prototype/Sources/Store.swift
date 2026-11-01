import SwiftUI
import AppKit
import ServiceManagement

@MainActor
final class Store: ObservableObject {
    // Saved to ~/Library/Application Support/SkillDock/data.json
    @Published var favorites: [Favorite] = [] { didSet { save() } }
    @Published var projects: [Project] = [] { didSet { save() } }
    @Published var settings = AppSettings() { didSet { save() } }
    @Published var selectedProjectID: UUID? { didSet { save() } }
    @Published var installed: [String: String] = [:] { didSet { save() } }

    // Session state
    @Published var selection: Set<UUID> = []
    @Published var remote: [UUID: String] = [:]          // latest version on GitHub per favorite
    @Published var lastChecked: Date?
    @Published var isChecking = false
    @Published var log = ""
    @Published var status = ""
    @Published var lastFailed = false
    @Published var isRunning = false
    @Published var isResolving = false
    @Published var addError: String?

    static let dataURL: URL = {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let dir = base.appendingPathComponent("SkillDock", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("data.json")
    }()

    init() {
        if let data = try? Data(contentsOf: Self.dataURL),
           let saved = try? JSONDecoder().decode(SavedState.self, from: data) {
            favorites = saved.favorites
            projects = saved.projects
            settings = saved.settings
            selectedProjectID = saved.selectedProjectID ?? saved.projects.first?.id
            installed = saved.installed
        }
        if settings.autoCheck { Notifier.requestPermission() }
        startAutoCheck()
    }

    private func save() {
        var state = SavedState()
        state.favorites = favorites
        state.projects = projects
        state.settings = settings
        state.selectedProjectID = selectedProjectID
        state.installed = installed
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? enc.encode(state) { try? data.write(to: Self.dataURL, options: .atomic) }
    }

    // MARK: - Target (project or global)

    var selectedProject: Project? { projects.first { $0.id == selectedProjectID } }

    var targetName: String {
        if settings.global { return "Global" }
        return selectedProject?.name ?? "Add a project"
    }

    /// Key prefix for the install record: project id, or "global".
    var currentScope: String? {
        settings.global ? "global" : selectedProjectID?.uuidString
    }

    func selectProject(_ p: Project) {
        selectedProjectID = p.id
        settings.global = false
    }

    func pickProjects() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = true
        panel.prompt = "Add project"
        panel.message = "Choose one or more project folders"
        NSApp.activate(ignoringOtherApps: true)
        if panel.runModal() == .OK { addProjects(panel.urls) }
    }

    func addProjects(_ urls: [URL]) {
        var lastAdded: UUID?
        for url in urls {
            var isDir: ObjCBool = false
            guard FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir), isDir.boolValue else { continue }
            if let existing = projects.first(where: { $0.path == url.path }) {
                lastAdded = existing.id
                continue
            }
            let p = Project(path: url.path)
            projects.append(p)
            lastAdded = p.id
        }
        if let lastAdded {
            selectedProjectID = lastAdded
            settings.global = false
        }
    }

    func removeSelectedProject() {
        guard let id = selectedProjectID else { return }
        projects.removeAll { $0.id == id }
        installed = installed.filter { !$0.key.hasPrefix(id.uuidString + "|") }
        selectedProjectID = projects.first?.id
    }

    func revealSelectedProject() {
        guard let p = selectedProject else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: p.path)])
    }

    // MARK: - Favorites

    /// Returns true when at least one favorite was added.
    func addFavorites(from text: String) async -> Bool {
        addError = nil
        var found = SourceParser.parse(text)
        if found.isEmpty, SourceParser.isUISkillsURL(text) {
            isResolving = true
            if let f = await SourceParser.resolveUISkills(text) { found = [f] }
            isResolving = false
            if found.isEmpty {
                addError = "Couldn't find the repo on that page. Paste owner/repo instead."
                return false
            }
        }
        guard !found.isEmpty else {
            addError = "Couldn't read that. Try a GitHub or skills.sh link, or owner/repo."
            return false
        }
        var added = 0
        for f in found where !favorites.contains(where: { $0.source == f.source && $0.skill == f.skill }) {
            favorites.append(f)
            selection.insert(f.id)
            added += 1
        }
        if added == 0 { addError = "Already in your favorites." }
        return added > 0
    }

    func remove(_ fav: Favorite) {
        favorites.removeAll { $0.id == fav.id }
        selection.remove(fav.id)
        remote[fav.id] = nil
        installed = installed.filter { !$0.key.hasSuffix("|" + fav.id.uuidString) }
    }

    func rename(_ fav: Favorite, to name: String) {
        let n = name.trimmingCharacters(in: .whitespaces)
        guard !n.isEmpty, let i = favorites.firstIndex(where: { $0.id == fav.id }) else { return }
        favorites[i].name = n
    }

    func toggle(_ fav: Favorite) {
        if selection.contains(fav.id) { selection.remove(fav.id) } else { selection.insert(fav.id) }
    }

    func copyCommand(_ fav: Favorite) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(Installer.command(for: fav, settings: settings), forType: .string)
        setStatus("Command copied")
    }

    // MARK: - Install state and updates

    private func key(_ fav: Favorite, _ scope: String) -> String { "\(scope)|\(fav.id.uuidString)" }

    func state(of fav: Favorite) -> SkillState {
        guard let scope = currentScope, let have = installed[key(fav, scope)] else { return .notInstalled }
        if have != "?", let latest = remote[fav.id], latest != have { return .updateAvailable }
        return .installed
    }

    /// Updates for the project you're looking at.
    var updatesHere: [Favorite] { favorites.filter { state(of: $0) == .updateAvailable } }

    /// Updates across every project (drives the menu bar icon).
    var totalUpdates: Int {
        installed.reduce(0) { count, entry in
            guard entry.value != "?",
                  let favID = entry.key.split(separator: "|").last.flatMap({ UUID(uuidString: String($0)) }),
                  let latest = remote[favID], latest != entry.value else { return count }
            return count + 1
        }
    }

    func checkForUpdates(notify: Bool = false) async {
        guard !isChecking, !favorites.isEmpty else { return }
        isChecking = true
        defer { isChecking = false }

        let before = totalUpdates
        let pairs: [(Favorite, UpdateChecker.Ref)] = favorites.compactMap { fav in
            UpdateChecker.ref(for: fav).map { (fav, $0) }
        }
        var trees: [UpdateChecker.Repo: UpdateChecker.Tree] = [:]
        var rateLimited = false
        for repo in Set(pairs.map { $0.1.repo }) {
            do {
                trees[repo] = try await UpdateChecker.fetchTree(repo)
            } catch UpdateChecker.CheckError.rateLimited {
                rateLimited = true
                break
            } catch {
                continue   // repo renamed, private, or offline: skip it
            }
        }
        for (fav, ref) in pairs {
            if let tree = trees[ref.repo], let v = UpdateChecker.version(of: fav, ref: ref, in: tree) {
                remote[fav.id] = v
            }
        }
        lastChecked = Date()
        if rateLimited {
            setStatus("GitHub limit reached. Try again in an hour.", failed: true)
        } else if status.hasPrefix("GitHub limit") {
            status = ""
        }
        if notify, totalUpdates > before { Notifier.postUpdates(totalUpdates) }
    }

    private func latestVersion(of fav: Favorite) async -> String? {
        guard let ref = UpdateChecker.ref(for: fav),
              let tree = try? await UpdateChecker.fetchTree(ref.repo) else { return nil }
        return UpdateChecker.version(of: fav, ref: ref, in: tree)
    }

    private func startAutoCheck() {
        Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            while true {
                guard let self else { return }
                if self.settings.autoCheck { await self.checkForUpdates(notify: true) }
                try? await Task.sleep(nanoseconds: 6 * 3600 * 1_000_000_000)
            }
        }
    }

    // MARK: - Install / update

    var canInstall: Bool {
        !isRunning && !selection.isEmpty && currentScope != nil
    }

    func installSelected() {
        install(favorites.filter { selection.contains($0.id) })
    }

    /// Installs or updates. Running `skills add` again replaces the old copy with the latest one.
    func install(_ favs: [Favorite]) {
        guard !favs.isEmpty, !isRunning else { return }
        guard let scope = currentScope else {
            setStatus("Add a project first.", failed: true)
            return
        }
        let dir: String? = settings.global ? nil : selectedProject?.path
        let target = targetName
        let settingsSnapshot = settings
        isRunning = true
        lastFailed = false
        log = ""

        Task {
            var failed: [String] = []
            for (i, fav) in favs.enumerated() {
                status = favs.count == 1 ? "Installing \(fav.name)…" : "Installing \(i + 1) of \(favs.count)…"
                let cmd = Installer.command(for: fav, settings: settingsSnapshot)
                appendLog("$ \(cmd)\n")
                let code = await Installer.run(cmd, in: dir) { [weak self] text in
                    Task { @MainActor in self?.appendLog(text) }
                }
                appendLog("\n")
                if code == 0 {
                    let v = await latestVersion(of: fav)
                    installed[key(fav, scope)] = v ?? "?"
                    if let v { remote[fav.id] = v }
                } else {
                    failed.append(fav.name)
                }
            }
            isRunning = false
            if failed.isEmpty {
                setStatus("Installed \(favs.count) skill\(favs.count == 1 ? "" : "s") in \(target)")
            } else {
                setStatus("Couldn't install \(failed.joined(separator: ", ")). See log.", failed: true)
            }
        }
    }

    private func setStatus(_ text: String, failed: Bool = false) {
        status = text
        lastFailed = failed
    }

    private func appendLog(_ text: String) {
        log += text
        if log.count > 60_000 { log = String(log.suffix(60_000)) }
    }

    // MARK: - Misc

    var launchAtLogin: Bool { SMAppService.mainApp.status == .enabled }

    func setLaunchAtLogin(_ on: Bool) {
        do {
            if on { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
        } catch {
            setStatus("Move SkillDock to Applications to open it at login.", failed: true)
        }
        objectWillChange.send()
    }

    func revealDataFile() {
        NSWorkspace.shared.activateFileViewerSelecting([Self.dataURL])
    }
}
