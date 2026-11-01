import Foundation
import CryptoKit

/// Finds the current version of a skill on GitHub.
/// A "version" is the git hash of the skill's folder, so it only changes when that skill's files change.
enum UpdateChecker {

    struct Repo: Hashable {
        let owner: String
        let name: String
    }

    struct Ref {
        let repo: Repo
        /// Folder path inside the repo, when the favorite points at one directly (…/tree/main/skills/x).
        let folder: String?
    }

    struct Tree {
        /// folder path → git tree hash ("" = repo root)
        let dirs: [String: String]
        /// folders that contain a SKILL.md ("" = repo root)
        let skillDirs: [String]
    }

    enum CheckError: Error { case rateLimited, notFound }

    // MARK: Which repo does a favorite live in?

    static func ref(for fav: Favorite) -> Ref? {
        let s = fav.source
        if s.range(of: #"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$"#, options: .regularExpression) != nil {
            let p = s.split(separator: "/").map(String.init)
            return Ref(repo: Repo(owner: p[0], name: p[1]), folder: nil)
        }
        guard let url = URL(string: s), let host = url.host?.lowercased(),
              host == "github.com" || host == "www.github.com" else { return nil }
        let parts = url.pathComponents.filter { $0 != "/" }
        guard parts.count >= 2 else { return nil }
        let repo = Repo(owner: parts[0], name: parts[1].replacingOccurrences(of: ".git", with: ""))
        if parts.count >= 5, parts[2] == "tree" || parts[2] == "blob" {
            return Ref(repo: repo, folder: parts[4...].joined(separator: "/"))
        }
        return Ref(repo: repo, folder: nil)
    }

    // MARK: Version of one favorite inside a fetched tree

    static func version(of fav: Favorite, ref: Ref, in tree: Tree) -> String? {
        if let folder = ref.folder { return tree.dirs[folder] }

        if let skill = fav.skill, !skill.isEmpty {
            let want = normalize(skill)
            let match = tree.skillDirs.first { dir in
                let folderName = dir.isEmpty ? ref.repo.name : String(dir.split(separator: "/").last ?? "")
                return normalize(folderName) == want
            }
            guard let dir = match else { return nil }
            return tree.dirs[dir]
        }

        // Whole repo: combine every skill folder's hash, so README edits don't count as updates.
        let hashes = tree.skillDirs.sorted().compactMap { tree.dirs[$0] }
        guard !hashes.isEmpty else { return tree.dirs[""] }
        let digest = SHA256.hash(data: Data(hashes.joined(separator: ",").utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private static func normalize(_ s: String) -> String {
        s.lowercased().replacingOccurrences(of: " ", with: "-")
    }

    // MARK: GitHub API (no token needed; 60 requests/hour, one request per repo per check)

    static func fetchTree(_ repo: Repo) async throws -> Tree {
        do {
            return try await fetchTree(repo, ref: "HEAD")
        } catch CheckError.rateLimited {
            throw CheckError.rateLimited
        } catch {
            let branch = try await defaultBranch(repo)
            return try await fetchTree(repo, ref: branch)
        }
    }

    private struct TreeResponse: Decodable {
        struct Entry: Decodable {
            let path: String
            let type: String
            let sha: String
        }
        let sha: String
        let tree: [Entry]
    }

    private struct RepoResponse: Decodable {
        let default_branch: String
    }

    private static func fetchTree(_ repo: Repo, ref: String) async throws -> Tree {
        let url = URL(string: "https://api.github.com/repos/\(repo.owner)/\(repo.name)/git/trees/\(ref)?recursive=1")!
        let r = try JSONDecoder().decode(TreeResponse.self, from: try await get(url))
        var dirs: [String: String] = ["": r.sha]
        var skillDirs: [String] = []
        for e in r.tree {
            if e.type == "tree" { dirs[e.path] = e.sha }
            if e.type == "blob" {
                if e.path == "SKILL.md" { skillDirs.append("") }
                else if e.path.hasSuffix("/SKILL.md") { skillDirs.append(String(e.path.dropLast("/SKILL.md".count))) }
            }
        }
        return Tree(dirs: dirs, skillDirs: skillDirs)
    }

    private static func defaultBranch(_ repo: Repo) async throws -> String {
        let url = URL(string: "https://api.github.com/repos/\(repo.owner)/\(repo.name)")!
        return try JSONDecoder().decode(RepoResponse.self, from: try await get(url)).default_branch
    }

    private static func get(_ url: URL) async throws -> Data {
        var req = URLRequest(url: url)
        req.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        req.setValue("SkillDock", forHTTPHeaderField: "User-Agent")
        req.timeoutInterval = 20
        let (data, resp) = try await URLSession.shared.data(for: req)
        guard let http = resp as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        switch http.statusCode {
        case 200: return data
        case 403, 429: throw CheckError.rateLimited
        default: throw CheckError.notFound
        }
    }
}
