import SwiftUI
import AppKit

struct ContentView: View {
    @EnvironmentObject var store: Store
    @State private var newSource = ""
    @State private var showSettings = false
    @State private var showLog = false

    var body: some View {
        VStack(spacing: 0) {
            header
                .padding(.horizontal, 14)
                .padding(.top, 12)
                .padding(.bottom, 10)

            addField
                .padding(.horizontal, 14)

            if let err = store.addError {
                Text(err)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 16)
                    .padding(.top, 6)
            }

            list
                .padding(.top, 8)

            footer
                .padding(14)
        }
        .frame(width: 340)
        .dropDestination(for: URL.self) { urls, _ in
            store.addProjects(urls)
            return true
        }
    }

    // MARK: Header: project menu · refresh · settings

    private var header: some View {
        HStack(spacing: 12) {
            projectMenu
            Spacer()
            Button {
                Task { await store.checkForUpdates() }
            } label: {
                if store.isChecking {
                    ProgressView().controlSize(.mini)
                } else {
                    Image(systemName: "arrow.clockwise")
                }
            }
            .buttonStyle(.borderless)
            .foregroundStyle(.secondary)
            .help("Check for updates")

            Button { showSettings.toggle() } label: { Image(systemName: "gearshape") }
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
                .help("Settings")
                .popover(isPresented: $showSettings, arrowEdge: .bottom) {
                    SettingsView().environmentObject(store)
                }
        }
    }

    private var projectMenu: some View {
        Menu {
            ForEach(store.projects) { p in
                Button {
                    store.selectProject(p)
                } label: {
                    if !store.settings.global && p.id == store.selectedProjectID {
                        Label(p.name, systemImage: "checkmark")
                    } else {
                        Text(p.name)
                    }
                }
            }
            Button {
                store.settings.global = true
            } label: {
                if store.settings.global {
                    Label("Global (~/)", systemImage: "checkmark")
                } else {
                    Text("Global (~/)")
                }
            }
            Divider()
            Button("Add project…") { store.pickProjects() }
            if store.selectedProject != nil && !store.settings.global {
                Button("Show in Finder") { store.revealSelectedProject() }
                Button("Remove from list") { store.removeSelectedProject() }
            }
        } label: {
            HStack(spacing: 4) {
                Text(store.targetName)
                    .font(.system(size: 13, weight: .semibold))
                Image(systemName: "chevron.down")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(.secondary)
            }
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
    }

    // MARK: Paste field

    private var addField: some View {
        HStack(spacing: 8) {
            Image(systemName: "plus")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
            TextField("Paste a link or owner/repo", text: $newSource)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .onSubmit(add)
            if store.isResolving {
                ProgressView().controlSize(.mini)
            }
        }
        .padding(.horizontal, 10)
        .frame(height: 30)
        .background(RoundedRectangle(cornerRadius: 7).fill(Color.primary.opacity(0.05)))
    }

    private func add() {
        let text = newSource
        guard !text.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        Task {
            if await store.addFavorites(from: text) { newSource = "" }
        }
    }

    // MARK: Favorites

    @ViewBuilder
    private var list: some View {
        if store.favorites.isEmpty {
            VStack(spacing: 6) {
                Text("Save your favorite skills")
                    .font(.system(size: 13, weight: .medium))
                Text("Paste a GitHub, skills.sh, or ui-skills.com link above.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Button("Add anthropics/skills") {
                    Task { _ = await store.addFavorites(from: "anthropics/skills") }
                }
                .buttonStyle(.link)
                .font(.caption)
                .padding(.top, 2)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 28)
        } else {
            ScrollView {
                VStack(spacing: 0) {
                    ForEach(store.favorites) { fav in
                        FavoriteRow(fav: fav)
                    }
                }
                .padding(.horizontal, 6)
            }
            .frame(height: min(CGFloat(store.favorites.count) * 44, 308))
        }
    }

    // MARK: Footer

    private var installLabel: String {
        if store.isRunning { return "Installing…" }
        let n = store.selection.count
        if n == 0 { return "Select skills to install" }
        return "Install \(n) skill\(n == 1 ? "" : "s")"
    }

    private var footer: some View {
        VStack(spacing: 10) {
            let updates = store.updatesHere
            if !updates.isEmpty && !store.isRunning {
                HStack(spacing: 8) {
                    Circle().fill(Color.accentColor).frame(width: 6, height: 6)
                    Text("\(updates.count) update\(updates.count == 1 ? "" : "s") available")
                        .font(.system(size: 12))
                    Spacer()
                    Button("Update all") { store.install(updates) }
                        .buttonStyle(.link)
                        .font(.system(size: 12, weight: .medium))
                }
                .padding(.horizontal, 2)
            }

            Button { store.installSelected() } label: {
                Text(installLabel)
                    .font(.system(size: 13, weight: .medium))
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(!store.canInstall)

            HStack(spacing: 6) {
                if store.isRunning { ProgressView().controlSize(.mini) }
                Text(statusLine)
                    .font(.system(size: 11))
                    .foregroundStyle(store.lastFailed ? Color.red : Color.secondary)
                    .lineLimit(1)
                Spacer()
                if !store.log.isEmpty {
                    Button(showLog ? "Hide log" : "Log") { showLog.toggle() }
                        .buttonStyle(.link)
                        .font(.system(size: 11))
                }
            }

            if showLog && !store.log.isEmpty {
                ScrollView {
                    Text(store.log)
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(8)
                }
                .frame(height: 130)
                .background(RoundedRectangle(cornerRadius: 7).fill(Color.primary.opacity(0.04)))
            }
        }
    }

    private var statusLine: String {
        if !store.status.isEmpty { return store.status }
        if store.isChecking { return "Checking for updates…" }
        if let d = store.lastChecked {
            return "Checked \(d.formatted(.relative(presentation: .named)))"
        }
        return " "
    }
}

// MARK: - Row

struct FavoriteRow: View {
    @EnvironmentObject var store: Store
    let fav: Favorite

    @State private var hovering = false
    @State private var renaming = false
    @State private var draft = ""

    var body: some View {
        let selected = store.selection.contains(fav.id)
        let state = store.state(of: fav)

        HStack(spacing: 10) {
            Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                .font(.system(size: 15))
                .foregroundStyle(selected ? Color.accentColor : Color.secondary.opacity(0.45))

            VStack(alignment: .leading, spacing: 2) {
                if renaming {
                    TextField("Name", text: $draft)
                        .textFieldStyle(.plain)
                        .font(.system(size: 13, weight: .medium))
                        .onSubmit {
                            store.rename(fav, to: draft)
                            renaming = false
                        }
                } else {
                    Text(fav.name)
                        .font(.system(size: 13, weight: .medium))
                        .lineLimit(1)
                }
                Text(fav.sourceLabel)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            Spacer(minLength: 6)
            trailing(state)
        }
        .padding(.horizontal, 8)
        .frame(height: 44)
        .background(RoundedRectangle(cornerRadius: 7).fill(hovering ? Color.primary.opacity(0.05) : Color.clear))
        .contentShape(Rectangle())
        .onTapGesture { if !renaming { store.toggle(fav) } }
        .onHover { hovering = $0 }
        .contextMenu {
            Button(state == .updateAvailable ? "Update" : "Install only this") { store.install([fav]) }
            Button("Copy install command") { store.copyCommand(fav) }
            Button("Rename…") { draft = fav.name; renaming = true }
            Divider()
            Button("Remove") { store.remove(fav) }
        }
    }

    @ViewBuilder
    private func trailing(_ state: SkillState) -> some View {
        switch state {
        case .updateAvailable:
            Button("Update") { store.install([fav]) }
                .buttonStyle(.plain)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(Color.accentColor)
                .padding(.horizontal, 9)
                .padding(.vertical, 3)
                .background(Capsule().fill(Color.accentColor.opacity(0.12)))
                .disabled(store.isRunning)
        case .installed, .notInstalled:
            if hovering && !store.isRunning {
                Button { store.remove(fav) } label: {
                    Image(systemName: "xmark").font(.system(size: 10, weight: .semibold))
                }
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
                .help("Remove from favorites")
            } else if state == .installed {
                Image(systemName: "checkmark")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.tertiary)
                    .help("Installed here, up to date")
            }
        }
    }
}

// MARK: - Settings

struct SettingsView: View {
    @EnvironmentObject var store: Store

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Agents").font(.system(size: 11)).foregroundStyle(.secondary)
                TextField("claude-code, cursor", text: $store.settings.agents)
                    .textFieldStyle(.roundedBorder)
                Text("Comma-separated. Use * for all agents.")
                    .font(.system(size: 11))
                    .foregroundStyle(.tertiary)
            }

            Toggle("Check for updates automatically", isOn: $store.settings.autoCheck)
            Toggle("Open at login", isOn: Binding(
                get: { store.launchAtLogin },
                set: { store.setLaunchAtLogin($0) }
            ))

            Divider()

            HStack {
                Button("Favorites file") { store.revealDataFile() }
                    .buttonStyle(.link)
                Spacer()
                Button("Quit") { NSApp.terminate(nil) }
                    .buttonStyle(.link)
            }
            .font(.system(size: 12))
        }
        .toggleStyle(.switch)
        .controlSize(.small)
        .padding(14)
        .frame(width: 260)
    }
}
