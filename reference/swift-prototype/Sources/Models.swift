import Foundation

/// One saved skill (or a whole repo of skills) you want to install quickly.
struct Favorite: Identifiable, Codable, Hashable {
    var id = UUID()
    var name: String
    /// What goes after `npx skills add`: owner/repo, a GitHub/GitLab URL, or a well-known site URL.
    var source: String
    /// A single skill inside the source. nil = install every skill in the repo.
    var skill: String?

    /// Second line under the name in the list.
    var sourceLabel: String {
        if let skill, !skill.isEmpty { return source }
        return "\(source) · all skills"
    }
}

struct Project: Identifiable, Codable, Hashable {
    var id = UUID()
    var path: String
    var name: String { URL(fileURLWithPath: path).lastPathComponent }
}

struct AppSettings: Codable {
    /// Comma-separated agent ids passed with -a (claude-code, cursor, codex …). Empty = let the CLI detect.
    var agents = "claude-code"
    /// Install to your home folder (-g) instead of the selected project.
    var global = false
    /// Check GitHub for newer versions on launch and every 6 hours.
    var autoCheck = true

    init() {}

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        agents = try c.decodeIfPresent(String.self, forKey: .agents) ?? "claude-code"
        global = try c.decodeIfPresent(Bool.self, forKey: .global) ?? false
        autoCheck = try c.decodeIfPresent(Bool.self, forKey: .autoCheck) ?? true
    }
}

struct SavedState: Codable {
    var favorites: [Favorite] = []
    var projects: [Project] = []
    var settings = AppSettings()
    var selectedProjectID: UUID?
    /// "<projectID or global>|<favoriteID>" → version (GitHub folder hash) installed there. "?" = unknown.
    var installed: [String: String] = [:]

    init() {}

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        favorites = try c.decodeIfPresent([Favorite].self, forKey: .favorites) ?? []
        projects = try c.decodeIfPresent([Project].self, forKey: .projects) ?? []
        settings = try c.decodeIfPresent(AppSettings.self, forKey: .settings) ?? AppSettings()
        selectedProjectID = try c.decodeIfPresent(UUID.self, forKey: .selectedProjectID)
        installed = try c.decodeIfPresent([String: String].self, forKey: .installed) ?? [:]
    }
}

enum SkillState {
    case notInstalled, installed, updateAvailable
}
