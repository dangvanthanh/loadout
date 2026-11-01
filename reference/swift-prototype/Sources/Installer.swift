import Foundation

/// Builds and runs `npx skills add …` in a project folder.
enum Installer {

    static func command(for fav: Favorite, settings: AppSettings) -> String {
        var c = "npx -y skills add \(shq(fav.source))"
        if let s = fav.skill, !s.isEmpty { c += " --skill \(shq(s))" }
        for agent in agentList(settings.agents) { c += " -a \(shq(agent))" }
        if settings.global { c += " -g" }
        c += " -y"
        return c
    }

    static func agentList(_ raw: String) -> [String] {
        raw.split(whereSeparator: { $0 == "," || $0.isWhitespace })
            .map(String.init)
            .filter { !$0.isEmpty }
    }

    /// Runs the command through a login shell so Homebrew / nvm installs of Node are found.
    /// Streams output to `onOutput` and returns the exit code.
    static func run(_ command: String, in dir: String?, onOutput: @escaping (String) -> Void) async -> Int32 {
        await withCheckedContinuation { (cont: CheckedContinuation<Int32, Never>) in
            let p = Process()
            p.executableURL = URL(fileURLWithPath: "/bin/zsh")

            var script = #"export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.volta/bin:$PATH"; "#
            script += #"[ -s "$HOME/.nvm/nvm.sh" ] && . "$HOME/.nvm/nvm.sh" >/dev/null 2>&1; "#
            script += #"command -v npx >/dev/null || { echo "npx not found. Install Node.js (brew install node) and try again."; exit 127; }; "#
            if let dir { script += "cd \(shq(dir)) && " }
            script += command
            p.arguments = ["-lc", script]
            p.currentDirectoryURL = URL(fileURLWithPath: dir ?? NSHomeDirectory())

            var env = ProcessInfo.processInfo.environment
            env["NO_COLOR"] = "1"
            env["FORCE_COLOR"] = "0"
            env["DO_NOT_TRACK"] = "1"
            p.environment = env

            let pipe = Pipe()
            p.standardOutput = pipe
            p.standardError = pipe
            p.standardInput = FileHandle.nullDevice

            pipe.fileHandleForReading.readabilityHandler = { handle in
                let data = handle.availableData
                guard !data.isEmpty, let s = String(data: data, encoding: .utf8) else { return }
                onOutput(stripANSI(s))
            }
            p.terminationHandler = { proc in
                // Drain anything left in the pipe before reporting the exit code.
                pipe.fileHandleForReading.readabilityHandler = nil
                let rest = pipe.fileHandleForReading.readDataToEndOfFile()
                if !rest.isEmpty, let s = String(data: rest, encoding: .utf8) { onOutput(stripANSI(s)) }
                cont.resume(returning: proc.terminationStatus)
            }
            do {
                try p.run()
            } catch {
                onOutput("Could not start the shell: \(error.localizedDescription)\n")
                cont.resume(returning: -1)
            }
        }
    }

    /// Single-quote a value for zsh.
    static func shq(_ s: String) -> String {
        "'" + s.replacingOccurrences(of: "'", with: #"'\''"#) + "'"
    }

    private static let ansi = try? NSRegularExpression(pattern: "\u{1B}\\[[0-9;?]*[ -/]*[@-~]")

    static func stripANSI(_ s: String) -> String {
        guard let ansi else { return s }
        let ns = s as NSString
        return ansi.stringByReplacingMatches(in: s, range: NSRange(location: 0, length: ns.length), withTemplate: "")
            .replacingOccurrences(of: "\r", with: "\n")
    }
}
