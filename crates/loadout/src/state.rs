use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use gpui_kit::*;
use loadout_core::context::{self as signals, Context as Signals};
use loadout_core::installed::{self, Installed};
use loadout_core::installer::{self, InstallError};
use loadout_core::model::{Added, Favorite, SavedState, Uuid};
use loadout_core::stack::{self, StackTag};
use loadout_core::{agents, parser, project, provenance, storage, tags, ui_skills};

use crate::macos::frontmost::{self, Detection, FrontApp};
use crate::macos::login_item::{self, LoginItem};
use crate::macos::{self as mac};

/// Where the palette installs.
#[derive(Clone, Debug, Default)]
pub struct Target {
    /// Project root.
    pub root: Option<PathBuf>,
    /// The app the folder came from; `None` when falling back to the last project.
    pub from_app: Option<String>,
    /// Why detection fell back, phrased as what to do.
    pub notice: Option<SharedString>,
    pub detecting: bool,
    pub stack: Vec<StackTag>,
    /// Skills already in the project, from its lock file and agent folders.
    pub installed: Installed,
    /// Open file, git changes, recent files: a ranking boost (U2).
    pub signals: Signals,
}

/// Everything detection learns off the main thread.
struct Resolved {
    detection: Detection,
    root: Option<PathBuf>,
    stack: Vec<StackTag>,
    installed: Installed,
    signals: Signals,
}

pub struct AppState {
    pub data: SavedState,
    /// `None` when data.json couldn't be read: never overwrite what we couldn't load.
    data_path: Option<PathBuf>,
    pub target: Target,
    pub login_item: LoginItem,
    /// Last popup status line, e.g. why open at login failed.
    pub popup_notice: Option<SharedString>,
    /// Output of `skills` runs, newest last.
    pub log: Vec<SharedString>,
    pub installing: bool,
    /// "Installed 2 skills in shop-dashboard", shown in the palette footer.
    pub install_status: Option<SharedString>,
    _detect: Option<Task<()>>,
    _resolve: Option<Task<()>>,
    _install: Option<Task<()>>,
}

const LOG_LINES: usize = 2000;

enum InstallEvent {
    Line(String),
    Done {
        favorite: Favorite,
        result: Result<i32, InstallError>,
    },
}

pub struct GlobalAppState(pub Entity<AppState>);

impl Global for GlobalAppState {}

impl AppState {
    pub fn new() -> Self {
        let mut popup_notice = None;
        let path = storage::data_path();
        let prototype = storage::prototype_path();
        let (data, data_path) = match path
            .as_deref()
            .map(|p| storage::load(p, prototype.as_deref()))
        {
            Some(Ok(loaded)) => {
                if let Some(storage::LoadNote::RecoveredCorrupt { backup }) = &loaded.note {
                    popup_notice = Some(
                        format!("Couldn't read data.json. Kept it as {}.", backup.display()).into(),
                    );
                }
                (loaded.state, path)
            }
            Some(Err(err)) => {
                popup_notice = Some(format!("{err}. Changes won't be saved.").into());
                (SavedState::default(), None)
            }
            None => (SavedState::default(), None),
        };
        Self {
            data,
            data_path,
            target: Target::default(),
            login_item: login_item::status(),
            popup_notice,
            log: Vec::new(),
            installing: false,
            install_status: None,
            _detect: None,
            _resolve: None,
            _install: None,
        }
    }

    fn save(&mut self) {
        let Some(path) = &self.data_path else { return };
        if let Err(err) = storage::save(path, &self.data) {
            self.popup_notice = Some(err.to_string().into());
        }
    }

    /// The most recently used project.
    fn last_project(&self) -> Option<PathBuf> {
        let selected = self.data.selected_project_id;
        self.data
            .projects
            .iter()
            .max_by_key(|p| (p.last_used, Some(p.id) == selected))
            .map(|p| p.path.clone())
    }

    /// Records `root` as the current project (prototype-compatible `projects` list).
    fn remember_project(&mut self, root: &std::path::Path) {
        let id = self.data.project_id(root);
        if let Some(project) = self.data.projects.iter_mut().find(|p| p.id == id) {
            project.last_used = Some(provenance::now());
        }
        self.data.selected_project_id = Some(id);
        self.save();
    }

    /// The install scope for the current target: its project id, or `None` for global.
    fn scope(&self) -> Option<Uuid> {
        let root = self
            .target
            .root
            .as_deref()
            .filter(|_| !self.data.settings.global)?;
        self.data
            .projects
            .iter()
            .find(|p| p.path == root)
            .map(|p| p.id)
    }

    /// Whether `favorite` is in the current target: recorded by Loadout, or found in the
    /// project's lock file / agent folders.
    pub fn is_installed_here(&self, favorite: Uuid) -> bool {
        let global = self.data.settings.global;
        if !global && self.target.root.is_none() {
            return false;
        }
        self.data.is_installed(self.scope(), favorite)
            || (!global
                && self
                    .data
                    .favorites
                    .iter()
                    .find(|f| f.id == favorite)
                    .is_some_and(|f| self.target.installed.contains(f)))
    }

    fn push_log(&mut self, line: impl Into<SharedString>) {
        self.log.push(line.into());
        if self.log.len() > LOG_LINES {
            self.log.drain(..self.log.len() - LOG_LINES);
        }
    }

    /// Installs `ids` into the current target with `npx skills add`, one after another, off the
    /// main thread. Skips favorites already installed there. Output streams into `log`.
    pub fn install(&mut self, ids: &[Uuid], cx: &mut Context<Self>) {
        if self.installing {
            return;
        }
        let global = self.data.settings.global;
        let root = self.target.root.clone();
        let place = match (&root, global) {
            (_, true) => "Global".to_string(),
            (Some(root), false) => root.file_name().map_or_else(
                || root.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
            (None, false) => {
                self.install_status = Some("No project to install into.".into());
                cx.notify();
                return;
            }
        };
        let scope = match (&root, global) {
            (Some(root), false) => Some(self.data.project_id(root)),
            _ => None,
        };
        let todo: Vec<Favorite> = self
            .data
            .favorites
            .iter()
            .filter(|f| ids.contains(&f.id) && !self.is_installed_here(f.id))
            .cloned()
            .collect();
        if todo.is_empty() {
            self.install_status = Some("Already installed".into());
            cx.notify();
            return;
        }

        let settings = self.data.settings.clone();
        let jobs: Vec<(Favorite, String)> = todo
            .into_iter()
            .map(|f| {
                let command = installer::install_command(&f, &settings);
                (f, command)
            })
            .collect();
        self.install_status = Some(match jobs.as_slice() {
            [(one, _)] => format!("Installing {}…", one.name).into(),
            many => format!("Installing {} skills…", many.len()).into(),
        });
        self.installing = true;
        cx.notify();

        let dir = if global { None } else { root.clone() };
        let (tx, rx) = mpsc::channel();
        let work_dir = dir.clone();
        let work = cx.background_executor().spawn(async move {
            for (favorite, command) in jobs {
                let _ = tx.send(InstallEvent::Line(format!("$ {command}")));
                let result = installer::run(&command, work_dir.as_deref(), |line| {
                    let _ = tx.send(InstallEvent::Line(line.to_string()));
                });
                let _ = tx.send(InstallEvent::Done { favorite, result });
            }
        });
        let agent = Some(settings.agents.clone()).filter(|a| !a.trim().is_empty());
        self._install = Some(cx.spawn(async move |this, cx| {
            let _work = work;
            let (mut installed, mut failed, mut npx_missing) = (Vec::new(), Vec::new(), false);
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let mut events = Vec::new();
                let finished = loop {
                    match rx.try_recv() {
                        Ok(event) => events.push(event),
                        Err(mpsc::TryRecvError::Empty) => break false,
                        Err(mpsc::TryRecvError::Disconnected) => break true,
                    }
                };
                let alive = this.update(cx, |this, cx| {
                    for event in events {
                        match event {
                            InstallEvent::Line(line) => this.push_log(line),
                            InstallEvent::Done { favorite, result } => match result {
                                Ok(0) => {
                                    this.data.mark_installed(scope, favorite.id, None);
                                    let mut entry = provenance::entry(
                                        provenance::INSTALL,
                                        dir.as_deref(),
                                        &favorite,
                                    );
                                    entry.agent = agent.clone();
                                    provenance::append(&mut this.data, entry);
                                    installed.push(favorite.name);
                                }
                                Ok(code) => {
                                    this.push_log(format!("Exited with code {code}"));
                                    failed.push(favorite.name);
                                }
                                Err(err) => {
                                    npx_missing |= matches!(err, InstallError::NpxMissing);
                                    this.push_log(err.to_string());
                                    failed.push(favorite.name);
                                }
                            },
                        }
                    }
                    if finished {
                        if let Some(root) = dir.as_deref() {
                            let ids = agents::parse_list(&this.data.settings.agents);
                            this.target.installed = installed::read(root, &ids);
                        }
                        this.installing = false;
                        this.install_status =
                            Some(install_summary(&installed, &failed, npx_missing, &place).into());
                        this.save();
                    }
                    cx.notify();
                });
                if finished || alive.is_err() {
                    break;
                }
            }
        }));
    }

    /// Resolves the target for `app` off the main thread; the last project stands in meanwhile.
    pub fn detect_target(&mut self, app: Option<FrontApp>, cx: &mut Context<Self>) {
        let fallback = self.last_project();
        if !self.installing {
            self.install_status = None;
        }
        self.target = Target {
            root: fallback.clone(),
            detecting: true,
            ..Default::default()
        };
        cx.notify();
        let agent_ids = agents::parse_list(&self.data.settings.agents);
        let task = cx.background_executor().spawn({
            let app = app.clone();
            async move {
                let detection = app
                    .as_ref()
                    .map_or(Detection::Unsupported, frontmost::detect);
                let (root, document) = match &detection {
                    Detection::Folder { folder, document } => {
                        (Some(project::root_for(folder)), document.clone())
                    }
                    _ => (fallback, None),
                };
                let Some(dir) = root.as_deref() else {
                    return Resolved {
                        detection,
                        root,
                        stack: Vec::new(),
                        installed: Installed::default(),
                        signals: Signals::default(),
                    };
                };
                Resolved {
                    stack: stack::detect(dir),
                    installed: installed::read(dir, &agent_ids),
                    signals: signals::gather(dir, document.as_deref()),
                    detection,
                    root,
                }
            }
        });
        // Replacing the task cancels a detection still running for an earlier palette.
        self._detect = Some(cx.spawn(async move |this, cx| {
            let resolved = task.await;
            this.update(cx, |this, cx| {
                this.apply_detection(app.as_ref(), resolved);
                cx.notify();
            })
            .ok();
        }));
    }

    fn apply_detection(&mut self, app: Option<&FrontApp>, resolved: Resolved) {
        let app_name = app.map_or_else(String::new, |app| app.name.clone());
        self.target = Target {
            root: resolved.root,
            stack: resolved.stack,
            installed: resolved.installed,
            signals: resolved.signals,
            ..Default::default()
        };
        match resolved.detection {
            Detection::Folder { .. } => {
                self.target.from_app = Some(app_name);
                if let Some(root) = self.target.root.clone() {
                    self.remember_project(&root);
                }
            }
            Detection::NeedsAutomation => {
                self.target.notice = Some(
                    "Couldn't read the Finder window. Allow Automation in System Settings.".into(),
                );
            }
            Detection::NeedsAccessibility => {
                self.target.notice = Some(
                    format!(
                        "Couldn't read the {app_name} window. Allow Accessibility in System Settings."
                    )
                    .into(),
                );
            }
            Detection::Unsupported => {}
        }
    }

    pub fn set_open_at_login(&mut self, on: bool, cx: &mut Context<Self>) {
        self.popup_notice = None;
        match login_item::set(on) {
            Ok(status) => {
                self.login_item = status;
                if status == LoginItem::NeedsApproval {
                    self.popup_notice =
                        Some("Allow Loadout in System Settings → General → Login Items.".into());
                }
            }
            Err(err) => {
                self.popup_notice = Some(if mac::is_bundled() {
                    format!("Couldn't change open at login. {err}").into()
                } else {
                    "Open at login needs Loadout.app. Run packaging/bundle.sh --install.".into()
                });
            }
        }
        cx.notify();
    }

    pub fn send_test_notification(&mut self, cx: &mut Context<Self>) {
        self.popup_notice = (!mac::is_bundled())
            .then(|| "Notifications need Loadout.app. Run packaging/bundle.sh --install.".into());
        cx.show_system_notification(SystemNotification {
            tag: "test".into(),
            title: "Loadout".into(),
            body: "Notifications work.".into(),
            actions: Vec::new(),
        });
        cx.notify();
    }

    /// Pasted text → favorites. ui-skills.com pages are fetched off the main thread.
    pub fn add_from_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        if ui_skills::is_ui_skills_url(&text) {
            self.popup_notice = Some("Reading ui-skills.com…".into());
            cx.notify();
            let task = cx
                .background_executor()
                .spawn(async move { ui_skills::resolve(&text) });
            self._resolve = Some(cx.spawn(async move |this, cx| {
                let result = task.await;
                this.update(cx, |this, cx| {
                    match result {
                        Ok(Some(favorite)) => this.add_favorites(vec![favorite]),
                        Ok(None) => {
                            this.popup_notice =
                                Some("Couldn't find the repo on that ui-skills.com page.".into())
                        }
                        Err(_) => {
                            this.popup_notice =
                                Some("Couldn't load ui-skills.com. Check your connection.".into())
                        }
                    }
                    cx.notify();
                })
                .ok();
            }));
            return;
        }
        let parsed = parser::parse(&text);
        if parsed.is_empty() {
            self.popup_notice = Some(
                "Couldn't read that. Paste owner/repo, a GitHub or skills.sh link, or an npx skills add command."
                    .into(),
            );
        } else {
            self.add_favorites(parsed);
        }
        cx.notify();
    }

    fn add_favorites(&mut self, favorites: Vec<Favorite>) {
        let Added { added, skipped } = self.data.add_favorites(favorites);
        self.popup_notice = Some(match (added.as_slice(), skipped.as_slice()) {
            ([one], _) => format!("Added {one}").into(),
            ([], [one]) => format!("{one} is already a favorite").into(),
            ([], _) => "Already favorites".into(),
            (many, _) => format!("Added {} favorites", many.len()).into(),
        });
        self.save();
    }

    pub fn rename_favorite(&mut self, id: Uuid, name: &str, cx: &mut Context<Self>) {
        if self.data.rename_favorite(id, name) {
            self.save();
            cx.notify();
        }
    }

    pub fn set_favorite_tags(&mut self, id: Uuid, text: &str, cx: &mut Context<Self>) {
        if self.data.set_favorite_tags(id, tags::parse_tags(text)) {
            self.save();
            cx.notify();
        }
    }

    pub fn remove_favorite(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(favorite) = self.data.remove_favorite(id) {
            self.popup_notice = Some(format!("Removed {}", favorite.name).into());
            self.save();
            cx.notify();
        }
    }

    /// "Next.js, Tailwind" for the Into line.
    pub fn stack_summary(&self) -> String {
        stack::summary(&self.target.stack, 2)
    }
}

/// "Installed 2 skills in shop-dashboard", "Couldn't install tdd. See log.", …
fn install_summary(
    installed: &[String],
    failed: &[String],
    npx_missing: bool,
    place: &str,
) -> String {
    if npx_missing {
        return InstallError::NpxMissing.to_string();
    }
    let skills = |names: &[String]| match names {
        [one] => one.clone(),
        many => format!("{} skills", many.len()),
    };
    match (installed.is_empty(), failed.is_empty()) {
        (_, true) => format!("Installed {} in {place}", skills(installed)),
        (true, false) => format!("Couldn't install {}. See log.", skills(failed)),
        (false, false) => format!(
            "Installed {}; couldn't install {}. See log.",
            skills(installed),
            skills(failed)
        ),
    }
}
