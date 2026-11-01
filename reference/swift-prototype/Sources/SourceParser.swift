import Foundation

/// Turns whatever you paste into favorites. Understands:
///   owner/repo                         owner/repo@skill-name
///   https://github.com/owner/repo      https://github.com/owner/repo/tree/main/skills/x
///   https://skills.sh/owner/repo/skill https://skills.sh/site/domain.com/skill
///   https://ui-skills.com/skills/owner/skill   (resolved by reading the page)
///   npx skills add owner/repo --skill a --skill b
enum SourceParser {

    static func parse(_ raw: String) -> [Favorite] {
        let text = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return [] }
        if text.contains("skills add ") { return parseCommand(text) }
        if let one = parseOne(text) { return [one] }
        return []
    }

    // MARK: - Single source

    private static let skillsShReserved: Set<String> = [
        "trending", "hot", "official", "packs", "topic", "audits", "docs", "agent",
        "about", "contact", "privacy", "terms", "api"
    ]

    private static func parseOne(_ text: String) -> Favorite? {
        // owner/repo@skill
        if !text.contains("://"), !text.hasPrefix("git@"), text.contains("@") {
            let bits = text.split(separator: "@", maxSplits: 1).map(String.init)
            if bits.count == 2, isShorthand(bits[0]), !bits[1].isEmpty {
                return Favorite(name: bits[1], source: bits[0], skill: bits[1])
            }
        }
        // owner/repo
        if isShorthand(text) {
            let repo = String(text.split(separator: "/")[1])
            return Favorite(name: repo, source: text, skill: nil)
        }
        // git@github.com:owner/repo.git
        if text.hasPrefix("git@") {
            let name = text.split(separator: "/").last.map { String($0).replacingOccurrences(of: ".git", with: "") } ?? text
            return Favorite(name: name, source: text, skill: nil)
        }

        guard let url = URL(string: text), let host = url.host?.lowercased() else { return nil }
        let parts = url.pathComponents.filter { $0 != "/" }

        if host == "skills.sh" || host.hasSuffix(".skills.sh") {
            guard let first = parts.first else { return nil }
            if first == "site", parts.count >= 3 {
                return Favorite(name: parts[2], source: "https://\(parts[1])", skill: parts[2])
            }
            if skillsShReserved.contains(first) { return nil }
            if parts.count >= 3 {
                return Favorite(name: parts[2], source: "\(parts[0])/\(parts[1])", skill: parts[2])
            }
            if parts.count == 2 {
                return Favorite(name: parts[1], source: "\(parts[0])/\(parts[1])", skill: nil)
            }
            return nil
        }

        if host == "github.com" || host == "www.github.com" {
            if parts.count >= 4, parts[2] == "tree" || parts[2] == "blob" {
                // Direct path to one skill folder: the CLI accepts the full URL.
                return Favorite(name: parts.last ?? text, source: text, skill: nil)
            }
            if parts.count >= 2 {
                let repo = parts[1].replacingOccurrences(of: ".git", with: "")
                return Favorite(name: repo, source: "\(parts[0])/\(repo)", skill: nil)
            }
            return nil
        }

        if host.hasSuffix("gitlab.com") {
            return Favorite(name: parts.last ?? text, source: text, skill: nil)
        }
        return nil
    }

    private static func isShorthand(_ s: String) -> Bool {
        s.range(of: #"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$"#, options: .regularExpression) != nil
    }

    // MARK: - Pasted `npx skills add …` commands

    private static func parseCommand(_ s: String) -> [Favorite] {
        let t = tokenize(s)
        guard let addIdx = t.firstIndex(of: "add"), addIdx + 1 < t.count else { return [] }
        var source: String?
        var skills: [String] = []
        var i = addIdx + 1
        while i < t.count {
            let tok = t[i]
            if tok == "--skill" || tok == "-s" {
                i += 1
                while i < t.count, !t[i].hasPrefix("-") { skills.append(t[i]); i += 1 }
                continue
            }
            if tok == "-a" || tok == "--agent" {
                i += 1
                while i < t.count, !t[i].hasPrefix("-") { i += 1 }
                continue
            }
            if tok.hasPrefix("-") { i += 1; continue }
            if source == nil { source = tok }
            i += 1
        }
        guard let src = source else { return [] }
        let base = parseOne(src) ?? Favorite(name: src, source: src, skill: nil)
        let named = skills.filter { $0 != "*" }
        if named.isEmpty { return [base] }
        return named.map { Favorite(name: $0, source: base.source, skill: $0) }
    }

    private static func tokenize(_ s: String) -> [String] {
        var tokens: [String] = []
        var cur = ""
        var quote: Character?
        for ch in s {
            if let q = quote {
                if ch == q { quote = nil } else { cur.append(ch) }
            } else if ch == "\"" || ch == "'" {
                quote = ch
            } else if ch.isWhitespace {
                if !cur.isEmpty { tokens.append(cur); cur = "" }
            } else {
                cur.append(ch)
            }
        }
        if !cur.isEmpty { tokens.append(cur) }
        return tokens
    }

    // MARK: - ui-skills.com links

    static func isUISkillsURL(_ s: String) -> Bool {
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
        return URL(string: t)?.host?.lowercased().hasSuffix("ui-skills.com") == true
    }

    /// ui-skills.com/skills/<owner>/<skill> doesn't say which repo the skill lives in,
    /// so read the page and look for an install command or the author's GitHub repo.
    static func resolveUISkills(_ s: String) async -> Favorite? {
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let url = URL(string: t) else { return nil }
        let parts = url.pathComponents.filter { $0 != "/" }
        guard parts.count >= 3, parts[0] == "skills" else { return nil }
        let owner = parts[1].lowercased()
        let skill = parts[2]

        guard let response = try? await URLSession.shared.data(from: url),
              let html = String(data: response.0, encoding: .utf8) else { return nil }
        let page = html
            .replacingOccurrences(of: "&quot;", with: "\"")
            .replacingOccurrences(of: "&#39;", with: "'")
            .replacingOccurrences(of: "&#x27;", with: "'")
            .replacingOccurrences(of: "&amp;", with: "&")

        // 1) An install command printed on the page.
        for cmd in matches(#"npx skills add [^<\n]+"#, in: page) {
            let favs = parseCommand(cmd)
            if let f = favs.first(where: { $0.skill == skill }) ?? favs.first { return f }
        }
        // 2) A GitHub repo owned by the skill's author (skip the site's own repo link).
        for link in matches(#"github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+"#, in: page) {
            let p = link.split(separator: "/").map(String.init)
            guard p.count >= 3, p[1].lowercased() == owner else { continue }
            let repo = p[2].replacingOccurrences(of: ".git", with: "")
            return Favorite(name: skill, source: "\(p[1])/\(repo)", skill: skill)
        }
        return nil
    }

    private static func matches(_ pattern: String, in text: String) -> [String] {
        guard let re = try? NSRegularExpression(pattern: pattern) else { return [] }
        let ns = text as NSString
        return re.matches(in: text, range: NSRange(location: 0, length: ns.length)).map { ns.substring(with: $0.range) }
    }
}
