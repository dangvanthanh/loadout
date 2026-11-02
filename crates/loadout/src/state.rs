use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use gpui_kit::*;
use loadout_core::context::{self as signals, Context as Signals};
use loadout_core::diff::{self, Changes};
use loadout_core::installed::{self, Installed};
use loadout_core::installer::{self, InstallError};
use loadout_core::model::{Added, Favorite, Pending, ProvenanceEntry, SavedState, Uuid};
use loadout_core::safety::{self, Report};
use loadout_core::share::SharedLoadout;
use loadout_core::stack::{self, StackTag};
use loadout_core::updates::{self, Repo, Tree};
use loadout_core::{
    agents, digest, launch, loadout_diff, parser, project, provenance, share, storage, tags,
    ui_skills,
};

use crate::macos::frontmost::{self, Detection, FrontApp};
use crate::macos::login_item::{self, LoginItem};
use crate::macos::{self as mac};
use crate::tray::Tray;

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
    /// Picked in the project switcher rather than detected.
    pub chosen: bool,
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
    /// Favorite → latest version from the last check (session).
    pub latest: HashMap<Uuid, String>,
    /// Repo listings from the last check, reused by previews and diffs.
    pub trees: HashMap<Repo, Tree>,
    pub checking: bool,
    /// Unix seconds.
    pub last_checked: Option<u64>,
    pub check_error: Option<SharedString>,
    /// What changed in the update the popup is showing.
    pub what_changed: Option<WhatChanged>,
    /// Files + safety findings per favorite, for first-install glances and previews (session).
    pub scans: HashMap<Uuid, Scan>,
    /// Shown in the palette when a first install has findings; Return again installs anyway.
    pub safety_notice: Option<SafetyNotice>,
    /// An equip waiting for its scans (ids, launch); it runs or shows the notice when they land.
    awaiting: Option<(Vec<Uuid>, bool)>,
    _launch: Option<Task<()>>,
    /// A loadout from a link or file, waiting for Import in the popup.
    pub pending_import: Option<SharedLoadout>,
    _share: Option<Task<()>>,
    _scans: Vec<Task<()>>,
    _detect: Option<Task<()>>,
    _changes: Option<Task<()>>,
    _resolve: Option<Task<()>>,
    _install: Option<Task<()>>,
    _check: Option<Task<()>>,
}

const LOG_LINES: usize = 2000;
/// Background update checks, like the prototype: every 6 hours.
const AUTO_CHECK_HOURS: u32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckReason {
    /// ↻: no notification.
    Manual,
    /// The 6-hourly check: notifies new versions, unless the digest is on.
    Auto,
    /// The digest: one summary of everything pending.
    Digest,
}

pub enum Scan {
    Loading,
    Ready(Arc<SkillScan>),
    /// Why there's no preview, phrased for the user.
    Unavailable(SharedString),
}

/// A skill folder as fetched from GitHub's default branch.
pub struct SkillScan {
    /// (path inside the skill folder, text); SKILL.md first.
    pub files: Vec<(String, String)>,
    pub report: Report,
}

pub struct SafetyNotice {
    /// The equip it holds back.
    pub ids: Vec<Uuid>,
    /// That equip asked to open the agent afterwards.
    pub launch: bool,
    /// The first flagged skill, for "Review".
    pub favorite: Uuid,
    pub text: SharedString,
}

/// One pending update's changes, for the popup.
pub struct WhatChanged {
    pub favorite: Uuid,
    pub installed: String,
    pub latest: String,
    /// Projects behind, by name ("Global" for the home folder).
    pub places: Vec<String>,
    pub changes: ChangesState,
}

pub enum ChangesState {
    Loading,
    Ready(Changes),
    /// Why there's no file list, phrased for the user.
    Unavailable(SharedString),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobKind {
    Install,
    Update,
}

/// One `npx skills add` run.
struct Job {
    favorite: Favorite,
    kind: JobKind,
    /// `None` = global.
    scope: Option<Uuid>,
    /// `None` = home folder (global installs).
    dir: Option<PathBuf>,
    command: String,
    /// The loadout being applied that brought this skill in.
    loadout: Option<Uuid>,
}

enum JobEvent {
    Line(String),
    Done {
        job: Box<Job>,
        result: Result<i32, InstallError>,
        /// The skill's version right after a successful run.
        version: Option<String>,
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
            latest: HashMap::new(),
            trees: HashMap::new(),
            checking: false,
            last_checked: None,
            check_error: None,
            what_changed: None,
            scans: HashMap::new(),
            safety_notice: None,
            awaiting: None,
            _launch: None,
            pending_import: None,
            _share: None,
            _scans: Vec::new(),
            _detect: None,
            _changes: None,
            _resolve: None,
            _install: None,
            _check: None,
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

    /// The current target's install scope: `Some(None)` = global, `Some(Some(id))` = a saved
    /// project, `None` = nowhere to install.
    fn target_scope(&self) -> Option<Option<Uuid>> {
        if self.data.settings.global {
            return Some(None);
        }
        let root = self.target.root.as_deref()?;
        let project = self.data.projects.iter().find(|p| p.path == root)?;
        Some(Some(project.id))
    }

    /// Whether `favorite` is in the current target: recorded by Loadout, or found in the
    /// project's lock file / agent folders.
    pub fn is_installed_here(&self, favorite: Uuid) -> bool {
        let global = self.data.settings.global;
        if !global && self.target.root.is_none() {
            return false;
        }
        let recorded = self
            .target_scope()
            .is_some_and(|scope| self.data.is_installed(scope, favorite));
        recorded
            || (!global
                && self
                    .data
                    .favorites
                    .iter()
                    .find(|f| f.id == favorite)
                    .is_some_and(|f| self.target.installed.contains(f)))
    }

    /// Recorded installs behind the latest version (pin / ignore applied).
    pub fn pending(&self) -> Vec<Pending> {
        self.data.pending_updates(&self.latest)
    }

    /// Whether `favorite` has an update in the current target.
    pub fn update_here(&self, favorite: Uuid) -> bool {
        let Some(scope) = self.target_scope() else {
            return false;
        };
        self.pending()
            .iter()
            .any(|p| p.favorite == favorite && p.project == scope)
    }

    fn push_log(&mut self, line: impl Into<SharedString>) {
        self.log.push(line.into());
        if self.log.len() > LOG_LINES {
            self.log.drain(..self.log.len() - LOG_LINES);
        }
    }

    /// `npx skills add …` for one scope: `-g` exactly when the scope is global.
    fn command_for(&self, favorite: &Favorite, global: bool) -> String {
        let mut settings = self.data.settings.clone();
        settings.global = global;
        installer::install_command(favorite, &settings)
    }

    /// Palette Return: installs `ids` into the current target, and updates the ones installed
    /// there with an update available. Others are skipped.
    /// With `launch`, opens the agent (`agent_for`) in the target afterwards (U9).
    pub fn equip(&mut self, ids: &[Uuid], launch: bool, cx: &mut Context<Self>) {
        if self.installing || ids.is_empty() {
            return;
        }
        // Return on a notice for the same selection: install anyway (and launch if either asked).
        let (anyway, launch) = match self.safety_notice.take() {
            Some(notice) if notice.ids == ids => (true, launch || notice.launch),
            _ => (false, launch),
        };
        self.awaiting = None;
        // The Into line may still show the fallback project; never install there by accident.
        if self.target.detecting && !self.data.settings.global {
            self.install_status = Some("Finding the project…".into());
            cx.notify();
            return;
        }
        let global = self.data.settings.global;
        let root = self.target.root.clone();
        let place = match (&root, global) {
            (_, true) => "Global".to_string(),
            (Some(root), false) => project_name(root),
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
        let dir = if global { None } else { root };
        let open_after = launch
            .then(|| self.agent_for(ids))
            .flatten()
            .map(|agent| (agent, dir.clone().or_else(dirs_home)));
        // Loadouts expand into their skills; a skill keeps the first loadout that asked for it.
        let mut wanted: Vec<(Uuid, Option<Uuid>)> = Vec::new();
        for id in ids {
            let members: Vec<(Uuid, Option<Uuid>)> = match self.data.loadout(*id) {
                Some(loadout) => loadout
                    .favorite_ids
                    .iter()
                    .map(|f| (*f, Some(*id)))
                    .collect(),
                None => vec![(*id, None)],
            };
            for (favorite, loadout) in members {
                match wanted.iter_mut().find(|(f, _)| *f == favorite) {
                    Some(entry) => entry.1 = entry.1.or(loadout),
                    None => wanted.push((favorite, loadout)),
                }
            }
        }
        let jobs: Vec<Job> = wanted
            .iter()
            .filter_map(|(id, loadout)| {
                Some((self.data.favorites.iter().find(|f| f.id == *id)?, *loadout))
            })
            .filter_map(|(f, loadout)| {
                let kind = if !self.is_installed_here(f.id) {
                    JobKind::Install
                } else if self.update_here(f.id) {
                    JobKind::Update
                } else {
                    return None;
                };
                Some(Job {
                    favorite: f.clone(),
                    kind,
                    scope,
                    dir: dir.clone(),
                    command: self.command_for(f, global),
                    loadout,
                })
            })
            .collect();
        if jobs.is_empty() {
            self.install_status = Some("Already installed".into());
            if let Some((agent, Some(dir))) = open_after {
                self.open_agent(agent, dir, None, cx);
            }
            cx.notify();
            return;
        }
        if !anyway && self.hold_for_safety(ids, launch, &jobs, cx) {
            return;
        }
        self.run_jobs(jobs, Some(place), open_after, cx);
    }

    /// The agent an equip of `ids` would open: the first selected loadout's preferred agent,
    /// else the first configured agent.
    pub fn agent_for(&self, ids: &[Uuid]) -> Option<String> {
        let preferred: Vec<Option<String>> = ids
            .iter()
            .filter_map(|id| self.data.loadout(*id))
            .map(|l| l.preferred_agent.clone())
            .collect();
        launch::pick_agent(&preferred, &self.data.settings.agents)
    }

    /// Opens `agent` in `dir` off the main thread. `done` (what the batch did) leads the status.
    fn open_agent(
        &mut self,
        agent: String,
        dir: PathBuf,
        done: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let custom = self.data.settings.launchers.clone();
        let name = launch::agent_name(&agent).to_string();
        let task = cx
            .background_executor()
            .spawn(async move { mac::launch::open(&agent, &dir, &custom) });
        self._launch = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.install_status = Some(
                    match (done, result) {
                        (Some(done), Ok(())) => format!("{done} · opened {name}"),
                        (None, Ok(())) => format!("Opened {name}"),
                        (Some(done), Err(err)) => format!("{done}. {err}"),
                        (None, Err(err)) => err,
                    }
                    .into(),
                );
                cx.notify();
            })
            .ok();
        }));
    }

    /// First installs get a glance before running (U4): waits for their scans, then shows a
    /// notice if any has findings. True when the equip is held back. Never blocks for good:
    /// a scan that fails counts as no findings, and Return on the notice installs anyway.
    fn hold_for_safety(
        &mut self,
        ids: &[Uuid],
        launch: bool,
        jobs: &[Job],
        cx: &mut Context<Self>,
    ) -> bool {
        let first: Vec<Uuid> = jobs
            .iter()
            .filter(|j| j.kind == JobKind::Install && self.needs_glance(j.favorite.id))
            .map(|j| j.favorite.id)
            .collect();
        self.ensure_scans(&first, cx);
        let loading = first
            .iter()
            .filter(|id| matches!(self.scans.get(id), Some(Scan::Loading)))
            .count();
        if loading > 0 {
            self.awaiting = Some((ids.to_vec(), launch));
            self.install_status = Some(match loading {
                1 => "Checking the skill…".into(),
                n => format!("Checking {n} skills…").into(),
            });
            cx.notify();
            return true;
        }
        let flagged: Vec<(Uuid, String)> = first
            .iter()
            .filter_map(|id| match self.scans.get(id) {
                Some(Scan::Ready(scan)) => Some((*id, scan.report.summary()?)),
                _ => None,
            })
            .collect();
        let Some((favorite, summary)) = flagged.first() else {
            return false;
        };
        let mut text = format!("{} {summary}.", self.favorite_name(*favorite));
        match flagged.len() {
            1 => {}
            2 => text += " 1 more skill has findings.",
            n => text += &format!(" {} more skills have findings.", n - 1),
        }
        self.safety_notice = Some(SafetyNotice {
            ids: ids.to_vec(),
            launch,
            favorite: *favorite,
            text: text.into(),
        });
        self.install_status = None;
        cx.notify();
        true
    }

    /// A skill Loadout never installed anywhere, from GitHub (others can't be read).
    pub fn needs_glance(&self, favorite: Uuid) -> bool {
        !self.data.ever_installed(favorite)
            && !self.is_installed_here(favorite)
            && self
                .data
                .favorites
                .iter()
                .find(|f| f.id == favorite)
                .and_then(updates::skill_ref)
                .is_some()
    }

    /// Fetches and scans the skills in `ids` not scanned yet this session (off the main thread):
    /// one listing per repo (reusing the update check's) and raw files, which cost no API quota.
    pub fn ensure_scans(&mut self, ids: &[Uuid], cx: &mut Context<Self>) {
        let todo: Vec<Favorite> = self
            .data
            .favorites
            .iter()
            .filter(|f| ids.contains(&f.id) && !self.scans.contains_key(&f.id))
            .cloned()
            .collect();
        if todo.is_empty() {
            return;
        }
        let mut trees = HashMap::new();
        for favorite in &todo {
            match updates::skill_ref(favorite) {
                Some(skill_ref) => {
                    if let Some(tree) = self.trees.get(&skill_ref.repo) {
                        trees.insert(skill_ref.repo, tree.clone());
                    }
                    self.scans.insert(favorite.id, Scan::Loading);
                }
                None => {
                    self.scans.insert(
                        favorite.id,
                        Scan::Unavailable("Preview needs a GitHub source.".into()),
                    );
                }
            }
        }
        let task = cx
            .background_executor()
            .spawn(async move { scan_skills(todo, trees) });
        self._scans.push(cx.spawn(async move |this, cx| {
            let (results, trees) = task.await;
            this.update(cx, |this, cx| {
                for (id, scan) in results {
                    this.scans.insert(id, scan);
                }
                this.trees.extend(trees);
                let ready = this.awaiting.as_ref().is_some_and(|(ids, _)| {
                    !ids.iter()
                        .any(|id| matches!(this.scans.get(id), Some(Scan::Loading)))
                });
                if ready && let Some((ids, launch)) = this.awaiting.take() {
                    this.equip(&ids, launch, cx);
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Updates every pending install of `only` (or of every skill), in each project it's in.
    pub fn update(&mut self, only: Option<Uuid>, cx: &mut Context<Self>) {
        if self.installing {
            return;
        }
        let (mut jobs, mut skipped) = (Vec::new(), 0);
        for p in self.pending() {
            if only.is_some_and(|id| id != p.favorite) {
                continue;
            }
            let Some(favorite) = self.data.favorites.iter().find(|f| f.id == p.favorite) else {
                continue;
            };
            let dir = match p.project {
                None => None,
                Some(id) => match self.data.projects.iter().find(|x| x.id == id) {
                    Some(project) if project.path.is_dir() => Some(project.path.clone()),
                    Some(project) => {
                        let line = format!(
                            "Skipped {}: {} is gone",
                            favorite.name,
                            project.path.display()
                        );
                        self.push_log(line);
                        skipped += 1;
                        continue;
                    }
                    None => continue,
                },
            };
            jobs.push(Job {
                favorite: favorite.clone(),
                kind: JobKind::Update,
                scope: p.project,
                command: self.command_for(favorite, p.project.is_none()),
                dir,
                loadout: None,
            });
        }
        if jobs.is_empty() {
            if skipped > 0 {
                self.install_status =
                    Some("Nothing to update: those project folders are gone. See log.".into());
            }
            cx.notify();
            return;
        }
        let place = match jobs.first().map(|j| j.dir.clone()) {
            Some(dir) if jobs.iter().all(|j| j.dir == dir) => Some(
                dir.as_deref()
                    .map_or_else(|| "Global".to_string(), project_name),
            ),
            _ => None,
        };
        self.run_jobs(jobs, place, None, cx);
    }

    /// Runs `jobs` one after another off the main thread, streaming output into `log`, then
    /// records versions and provenance.
    fn run_jobs(
        &mut self,
        jobs: Vec<Job>,
        place: Option<String>,
        mut open_after: Option<(String, Option<PathBuf>)>,
        cx: &mut Context<Self>,
    ) {
        let verb = if jobs.iter().all(|j| j.kind == JobKind::Update) {
            "Updating"
        } else {
            "Installing"
        };
        self.install_status = Some(match jobs.as_slice() {
            [one] => format!("{verb} {}…", one.favorite.name).into(),
            many => format!("{verb} {} skills…", many.len()).into(),
        });
        self.installing = true;
        cx.notify();

        let (tx, rx) = mpsc::channel();
        let work = cx.background_executor().spawn(async move {
            // Listings fetched for versions, one per repo for the whole batch.
            let mut trees = HashMap::new();
            for job in jobs {
                let _ = tx.send(JobEvent::Line(format!("$ {}", job.command)));
                let result = installer::run(&job.command, job.dir.as_deref(), |line| {
                    let _ = tx.send(JobEvent::Line(line.to_string()));
                });
                let version = matches!(result, Ok(0))
                    .then(|| updates::version_now(&job.favorite, &mut trees, None))
                    .flatten();
                let _ = tx.send(JobEvent::Done {
                    job: Box::new(job),
                    result,
                    version,
                });
            }
        });
        let agent = Some(self.data.settings.agents.clone()).filter(|a| !a.trim().is_empty());
        self._install = Some(cx.spawn(async move |this, cx| {
            let _work = work;
            let mut outcome = Outcome::default();
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
                            JobEvent::Line(line) => this.push_log(line),
                            JobEvent::Done {
                                job,
                                result,
                                version,
                            } => this.finish_job(
                                *job,
                                result,
                                version,
                                agent.as_deref(),
                                &mut outcome,
                            ),
                        }
                    }
                    if finished {
                        if let Some(root) = this.target.root.clone() {
                            let ids = agents::parse_list(&this.data.settings.agents);
                            this.target.installed = installed::read(&root, &ids);
                        }
                        this.installing = false;
                        this.record_applied(&outcome.applied);
                        let summary = outcome.summary(place.as_deref());
                        this.install_status = Some(summary.clone().into());
                        // Open the agent unless every job failed.
                        let worked = !outcome.installed.is_empty() || !outcome.updated.is_empty();
                        if worked && let Some((agent, Some(dir))) = open_after.take() {
                            this.open_agent(agent, dir, Some(summary), cx);
                        }
                        this.save();
                        this.refresh_badge(cx);
                    }
                    cx.notify();
                });
                if finished || alive.is_err() {
                    break;
                }
            }
        }));
    }

    fn finish_job(
        &mut self,
        job: Job,
        result: Result<i32, InstallError>,
        version: Option<String>,
        agent: Option<&str>,
        outcome: &mut Outcome,
    ) {
        let name = job.favorite.name.clone();
        match result {
            Ok(0) => {
                let id = job.favorite.id;
                if let Some(version) = &version {
                    self.latest.insert(id, version.clone());
                }
                let action = match job.kind {
                    JobKind::Install => {
                        self.data.mark_installed(job.scope, id, version.as_deref());
                        provenance::INSTALL
                    }
                    JobKind::Update => {
                        // Unknown right now: assume the version the check reported.
                        let version = version
                            .clone()
                            .or_else(|| self.latest.get(&id).cloned())
                            .unwrap_or_else(|| "?".into());
                        self.data.set_installed_version(job.scope, id, &version);
                        provenance::UPDATE
                    }
                };
                let mut entry = provenance::entry(action, job.dir.as_deref(), &job.favorite);
                entry.tree_sha = version;
                entry.agent = agent.map(str::to_string);
                if let Some(loadout) = job.loadout.and_then(|id| self.data.loadout(id)) {
                    entry.loadout_id = Some(loadout.id);
                    entry.note = Some(format!("from {}", loadout.name));
                    if !outcome.applied.iter().any(|(id, _)| *id == loadout.id) {
                        outcome.applied.push((loadout.id, job.dir.clone()));
                    }
                }
                provenance::append(&mut self.data, entry);
                match job.kind {
                    JobKind::Install => outcome.installed.push(name),
                    JobKind::Update => outcome.updated.push(name),
                }
            }
            Ok(code) => {
                self.push_log(format!("Exited with code {code}"));
                outcome.failed.push((name, job.kind));
            }
            Err(err) => {
                outcome.npx_missing |= matches!(err, InstallError::NpxMissing);
                self.push_log(err.to_string());
                outcome.failed.push((name, job.kind));
            }
        }
    }

    /// One `apply_loadout` provenance entry per loadout a batch applied.
    fn record_applied(&mut self, applied: &[(Uuid, Option<PathBuf>)]) {
        for (id, dir) in applied {
            let Some(loadout) = self.data.loadout(*id) else {
                continue;
            };
            let entry = ProvenanceEntry {
                id: Uuid::new_v4(),
                ts: provenance::now(),
                action: provenance::APPLY_LOADOUT.into(),
                project_path: dir.as_ref().map(|d| d.to_string_lossy().into_owned()),
                skill_source: loadout.name.clone(),
                loadout_id: Some(loadout.id),
                agent: loadout.preferred_agent.clone(),
                ..Default::default()
            };
            provenance::append(&mut self.data, entry);
        }
    }

    /// Saves `ids` (favorites, and loadouts' skills) as a new loadout, suggested for the current
    /// stack. Feedback goes to the palette footer. True when saved.
    pub fn create_loadout(&mut self, name: &str, ids: &[Uuid], cx: &mut Context<Self>) -> bool {
        let mut favorites: Vec<Uuid> = Vec::new();
        for id in ids {
            match self.data.loadout(*id) {
                Some(loadout) => favorites.extend(&loadout.favorite_ids),
                None => favorites.push(*id),
            }
        }
        // The Into line's stack, so the loadout is suggested for projects like this one.
        let tags = self
            .target
            .stack
            .iter()
            .take(2)
            .map(|t| t.tag.to_string())
            .collect();
        let result = self.data.create_loadout(name, &favorites, tags);
        cx.notify();
        match result {
            Ok(id) => {
                let count = self.data.loadout_favorites(id).len();
                self.install_status = Some(
                    format!(
                        "Saved {} ({count} skill{})",
                        name.trim(),
                        if count == 1 { "" } else { "s" }
                    )
                    .into(),
                );
                self.save();
                true
            }
            Err(err) => {
                self.install_status = Some(err.to_string().into());
                false
            }
        }
    }

    pub fn rename_loadout(&mut self, id: Uuid, name: &str, cx: &mut Context<Self>) {
        match self.data.rename_loadout(id, name) {
            Ok(()) => self.save(),
            Err(err) => self.popup_notice = Some(err.to_string().into()),
        }
        cx.notify();
    }

    pub fn set_loadout_tags(&mut self, id: Uuid, text: &str, cx: &mut Context<Self>) {
        if self.data.set_loadout_tags(id, tags::parse_tags(text)) {
            self.save();
            cx.notify();
        }
    }

    /// Accepts an agent id or name ("pi", "Claude Code"); empty clears it.
    pub fn set_loadout_agent(&mut self, id: Uuid, text: &str, cx: &mut Context<Self>) {
        let agent = match text.trim() {
            "" => None,
            text => match agents::resolve(text) {
                Some(agent) => Some(agent.id.to_string()),
                None => {
                    self.popup_notice =
                        Some("Unknown agent. Try pi, claude-code, cursor, or codex.".into());
                    cx.notify();
                    return;
                }
            },
        };
        if self.data.set_loadout_agent(id, agent) {
            self.save();
            cx.notify();
        }
    }

    pub fn toggle_loadout_favorite(&mut self, id: Uuid, favorite: Uuid, cx: &mut Context<Self>) {
        match self.data.toggle_loadout_favorite(id, favorite) {
            Ok(()) => self.save(),
            Err(err) => self.popup_notice = Some(err.to_string().into()),
        }
        cx.notify();
    }

    pub fn remove_loadout(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(loadout) = self.data.remove_loadout(id) {
            self.popup_notice = Some(format!("Removed {}", loadout.name).into());
            self.save();
            cx.notify();
        }
    }

    /// The current target versus `loadout`: missing, present, extra.
    pub fn loadout_diff(&self, loadout: Uuid) -> loadout_diff::LoadoutDiff {
        loadout_diff::diff(&self.data, loadout, &self.target.installed, |id| {
            self.is_installed_here(id)
        })
    }

    /// Validates a shared loadout and holds it for the popup's import preview.
    pub fn offer_import(&mut self, text: &str, cx: &mut Context<Self>) {
        match share::parse(text) {
            Ok(shared) => {
                self.pending_import = Some(shared);
                self.popup_notice = None;
            }
            Err(err) => self.popup_notice = Some(err.to_string().into()),
        }
        cx.notify();
    }

    /// Adds the pending loadout and the favorites it needs. Nothing is installed.
    pub fn confirm_import(&mut self, cx: &mut Context<Self>) {
        let Some(shared) = self.pending_import.take() else {
            return;
        };
        let imported = share::import(&mut self.data, &shared);
        provenance::append(&mut self.data, share::provenance_entry(&shared, &imported));
        let skills = match imported.new_favorites {
            0 => String::new(),
            1 => " and 1 new favorite".into(),
            n => format!(" and {n} new favorites"),
        };
        let verb = if imported.replaced {
            "Updated"
        } else {
            "Imported"
        };
        self.popup_notice = Some(format!("{verb} {}{skills}", imported.name).into());
        self.save();
        cx.notify();
    }

    /// Fetches a team loadout's source; previews it if it's newer than yours.
    pub fn check_loadout_source(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some((name, url)) = self
            .data
            .loadout(id)
            .and_then(|l| Some((l.name.clone(), l.source.clone()?)))
        else {
            return;
        };
        self.popup_notice = Some(format!("Checking {name}…").into());
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { share::fetch(&url) });
        self._share = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.popup_notice = match result {
                    Ok(shared) => match share::version_change(&this.data, &shared) {
                        share::VersionChange::Newer | share::VersionChange::New => {
                            this.pending_import = Some(shared);
                            None
                        }
                        _ => Some(
                            match &shared.version {
                                Some(v) => format!("{name} is up to date (version {v})"),
                                None => format!("{name} is up to date"),
                            }
                            .into(),
                        ),
                    },
                    Err(err) => Some(err.to_string().into()),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    /// Empty clears it.
    pub fn set_loadout_version(&mut self, id: Uuid, text: &str, cx: &mut Context<Self>) {
        let version = text.trim();
        if version.chars().any(char::is_control) || version.chars().count() > 80 {
            self.popup_notice = Some("That version has characters Loadout can't keep.".into());
        } else if let Some(loadout) = self.data.loadouts.iter_mut().find(|l| l.id == id) {
            loadout.version = (!version.is_empty()).then(|| version.to_string());
            self.save();
        }
        cx.notify();
    }

    /// An https URL to the published `.loadout.json`; empty clears it.
    pub fn set_loadout_source(&mut self, id: Uuid, text: &str, cx: &mut Context<Self>) {
        let url = text.trim();
        if !url.is_empty() && !share::is_https(url) {
            self.popup_notice = Some(share::ShareError::NotHttps.to_string().into());
        } else if let Some(loadout) = self.data.loadouts.iter_mut().find(|l| l.id == id) {
            loadout.source = (!url.is_empty()).then(|| url.to_string());
            self.save();
        }
        cx.notify();
    }

    pub fn cancel_import(&mut self, cx: &mut Context<Self>) {
        self.pending_import = None;
        cx.notify();
    }

    /// Copies a `loadout://` link to `id`.
    pub fn copy_loadout_link(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some(shared) = share::export(&self.data, id) else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(share::to_link(&shared)));
        self.popup_notice = Some(format!("Copied a link to {}", shared.name).into());
        cx.notify();
    }

    /// Asks where to save `<name>.loadout.json`, writes it, and shows it in Finder.
    pub fn save_loadout_file(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some(shared) = share::export(&self.data, id) else {
            return;
        };
        let file_name = format!("{}.loadout.json", slug(&shared.name));
        let folder = dirs_home().map(|h| h.join("Downloads")).unwrap_or_default();
        // An accessory app must come forward for the save panel to show.
        cx.activate(true);
        let chosen = cx.prompt_for_new_path(&folder, Some(&file_name));
        self._share = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let written = std::fs::write(&path, share::to_json(&shared));
            this.update(cx, |this, cx| {
                this.popup_notice = Some(match written {
                    Ok(()) => {
                        cx.reveal_path(&path);
                        format!(
                            "Saved {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        )
                        .into()
                    }
                    Err(err) => format!("Couldn't save the file: {err}").into(),
                });
                cx.notify();
            })
            .ok();
        }));
    }

    /// Asks for a `.loadout.json` and previews it in the popup.
    pub fn import_file(&mut self, cx: &mut Context<Self>) {
        cx.activate(true);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        self._share = Some(cx.spawn(async move |_, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.first() else { return };
            let text = mac::url_scheme::read_small(path).unwrap_or_default();
            cx.update(|cx| crate::windows::offer_import(&text, cx));
        }));
    }

    /// "4 skills · prefers Pi" for loadout rows.
    pub fn loadout_subtitle(&self, id: Uuid) -> String {
        let count = self.data.loadout_favorites(id).len();
        let mut text = format!("{count} skill{}", if count == 1 { "" } else { "s" });
        if let Some(agent) = self
            .data
            .loadout(id)
            .and_then(|l| l.preferred_agent.as_deref())
        {
            let name = agents::find(agent).map_or(agent, |a| a.name);
            text += &format!(" · prefers {name}");
        }
        text
    }

    /// Fetches the latest versions of installed skills (one GitHub request per repo, off the
    /// main thread). With `notify`, posts one notification for versions not announced before.
    pub fn check_for_updates(&mut self, reason: CheckReason, cx: &mut Context<Self>) {
        if self.checking {
            return;
        }
        let tracked = self.data.tracked_favorites();
        if tracked.is_empty() {
            self.last_checked = Some(provenance::now());
            if reason == CheckReason::Digest {
                self.data.last_digest = self.last_checked;
                self.save();
            }
            cx.notify();
            return;
        }
        self.checking = true;
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { updates::check(&tracked, None) });
        self._check = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.checking = false;
                this.last_checked = Some(provenance::now());
                this.check_error = result
                    .rate_limited
                    .then(|| updates::UpdateError::RateLimited.to_string().into());
                this.latest.extend(result.latest);
                this.trees.extend(result.trees);
                let pending = this.pending();
                let fresh = this.data.take_unnotified(&pending);
                let digest = this.data.settings.digest_enabled;
                match reason {
                    // With the digest on, it's the only notification.
                    CheckReason::Auto if !digest => this.notify_updates(&fresh, "updates", cx),
                    CheckReason::Digest => {
                        let all: Vec<Uuid> = pending.iter().map(|p| p.favorite).collect();
                        this.notify_updates(&all, "digest", cx);
                        this.data.last_digest = this.last_checked;
                    }
                    _ => {}
                }
                this.save();
                this.refresh_badge(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    /// One notification naming `ids`' skills; nothing when empty.
    fn notify_updates(&self, ids: &[Uuid], tag: &'static str, cx: &mut App) {
        let names: Vec<&str> = ids
            .iter()
            .filter_map(|id| self.data.favorites.iter().find(|f| f.id == *id))
            .map(|f| f.name.as_str())
            .collect();
        let Some(body) = digest::body(&names) else {
            return;
        };
        cx.show_system_notification(SystemNotification {
            tag: tag.into(),
            title: "Skill updates available".into(),
            body: body.into(),
            actions: Vec::new(),
        });
    }

    /// The menu bar dot follows whether anything is pending.
    fn refresh_badge(&self, cx: &mut App) {
        let updates = !self.pending().is_empty();
        if let Some(tray) = cx.try_global::<Tray>() {
            tray.set_updates(updates);
        }
    }

    /// Loads files + SKILL.md diff for `favorite`'s first pending update (four GitHub requests,
    /// off the main thread). Kept while the same update is shown again.
    pub fn show_changes(&mut self, favorite: Uuid, cx: &mut Context<Self>) {
        let pending: Vec<Pending> = self
            .pending()
            .into_iter()
            .filter(|p| p.favorite == favorite)
            .collect();
        let Some(first) = pending.first() else {
            self.what_changed = None;
            cx.notify();
            return;
        };
        if self.what_changed.as_ref().is_some_and(|w| {
            w.favorite == favorite && w.installed == first.installed && w.latest == first.latest
        }) {
            return;
        }
        let places = pending
            .iter()
            .map(|p| match p.project {
                None => "Global".to_string(),
                Some(id) => self
                    .data
                    .projects
                    .iter()
                    .find(|x| x.id == id)
                    .map_or_else(String::new, |x| project_name(&x.path)),
            })
            .collect();
        let (installed, latest) = (first.installed.clone(), first.latest.clone());
        let repo = self
            .data
            .favorites
            .iter()
            .find(|f| f.id == favorite)
            .and_then(updates::skill_ref)
            .map(|r| r.repo);
        let listable = updates::is_tree_sha(&installed) && updates::is_tree_sha(&latest);
        let changes = match (repo, listable) {
            (Some(repo), true) => {
                let (old, new) = (installed.clone(), latest.clone());
                let task = cx.background_executor().spawn(async move {
                    let old = updates::fetch_tree_at(&repo, &old, None)?;
                    let new = updates::fetch_tree_at(&repo, &new, None)?;
                    diff::skill_changes(&old, &new, |sha| updates::fetch_blob(&repo, sha, None))
                });
                self._changes = Some(cx.spawn(async move |this, cx| {
                    let result = task.await;
                    this.update(cx, |this, cx| {
                        if let Some(shown) = this
                            .what_changed
                            .as_mut()
                            .filter(|w| w.favorite == favorite)
                        {
                            shown.changes = match result {
                                Ok(changes) => ChangesState::Ready(changes),
                                Err(err) => ChangesState::Unavailable(
                                    format!("Couldn't load the changes: {err}.").into(),
                                ),
                            };
                        }
                        cx.notify();
                    })
                    .ok();
                }));
                ChangesState::Loading
            }
            _ => ChangesState::Unavailable(
                "Changes are listed for single skills only; this one is tracked as a whole repo."
                    .into(),
            ),
        };
        self.what_changed = Some(WhatChanged {
            favorite,
            installed,
            latest,
            places,
            changes,
        });
        cx.notify();
    }

    /// Pinned skills stay on their installed version: no pill, dot, or notification.
    pub fn set_pinned(&mut self, favorite: Uuid, pinned: bool, cx: &mut Context<Self>) {
        if self.data.set_pinned(favorite, pinned) {
            let name = self.favorite_name(favorite);
            self.popup_notice = Some(
                if pinned {
                    format!("Pinned {name}")
                } else {
                    format!("Unpinned {name}")
                }
                .into(),
            );
            self.save();
            self.refresh_badge(cx);
            cx.notify();
        }
    }

    /// Hides the current latest version of `favorite`; a newer one shows again.
    pub fn ignore_update(&mut self, favorite: Uuid, cx: &mut Context<Self>) {
        let Some(latest) = self.latest.get(&favorite).cloned() else {
            return;
        };
        if self.data.ignore_update(favorite, &latest) {
            self.popup_notice =
                Some(format!("Ignored this update to {}", self.favorite_name(favorite)).into());
            self.save();
            self.refresh_badge(cx);
            cx.notify();
        }
    }

    fn favorite_name(&self, favorite: Uuid) -> String {
        self.data
            .favorites
            .iter()
            .find(|f| f.id == favorite)
            .map_or_else(String::new, |f| f.name.clone())
    }

    /// Runs whatever background check is due: the digest (when on) or the 6-hourly check.
    /// Called hourly; cheap when nothing is due.
    pub fn tick(&mut self, cx: &mut Context<Self>) {
        let now = provenance::now();
        let settings = &self.data.settings;
        if settings.digest_enabled
            && digest::due(self.data.last_digest, settings.digest_interval_hours, now)
        {
            self.check_for_updates(CheckReason::Digest, cx);
        } else if settings.auto_check && digest::due(self.last_checked, AUTO_CHECK_HOURS, now) {
            self.check_for_updates(CheckReason::Auto, cx);
        }
    }

    /// `None` turns the digest off; otherwise the interval in hours.
    pub fn set_digest(&mut self, interval: Option<u32>, cx: &mut Context<Self>) {
        let settings = &mut self.data.settings;
        settings.digest_enabled = interval.is_some();
        if let Some(hours) = interval {
            settings.digest_interval_hours = hours;
            // Start counting from now rather than firing straight away.
            self.data.last_digest.get_or_insert(provenance::now());
        }
        self.save();
        cx.notify();
    }

    /// The palette shortcut, e.g. "ctrl+alt+L"; applied at once. False if it can't be used.
    pub fn set_hotkey(&mut self, spec: &str, cx: &mut Context<Self>) -> bool {
        let result = cx.update_global::<crate::hotkey::Hotkey, _>(|keys, _| keys.set(spec));
        match result {
            Ok(()) => {
                self.data.settings.hotkey = spec.trim().to_string();
                self.save();
                cx.notify();
                true
            }
            Err(_) => false,
        }
    }

    /// Agents passed with `-a`, e.g. "claude-code, cursor"; empty lets the CLI detect them.
    pub fn set_agents(&mut self, text: &str, cx: &mut Context<Self>) {
        self.data.settings.agents = agents::parse_list(text).join(", ");
        self.save();
        cx.notify();
    }

    pub fn set_launch_after_install(&mut self, on: bool, cx: &mut Context<Self>) {
        self.data.settings.launch_after_install = on;
        self.save();
        cx.notify();
    }

    pub fn set_auto_check(&mut self, on: bool, cx: &mut Context<Self>) {
        self.data.settings.auto_check = on;
        self.save();
        cx.notify();
    }

    /// "Checked 4 minutes ago" / "Checking for updates…" / the last error.
    pub fn check_status(&self) -> Option<SharedString> {
        if self.checking {
            return Some("Checking for updates…".into());
        }
        if let Some(error) = &self.check_error {
            return Some(error.clone());
        }
        self.last_checked
            .map(|at| format!("Checked {}", provenance::ago(at, provenance::now())).into())
    }

    /// Resolves the target for `app` off the main thread; the last project stands in meanwhile.
    pub fn detect_target(&mut self, app: Option<FrontApp>, cx: &mut Context<Self>) {
        self.resolve_target(app, None, cx);
    }

    /// The project switcher: `Some(folder)` targets that project, `None` installs globally (`-g`).
    /// Both persist: the project becomes the last used one; Global stays on until a project is
    /// picked.
    pub fn choose_target(&mut self, folder: Option<PathBuf>, cx: &mut Context<Self>) {
        match folder {
            None => {
                self.data.settings.global = true;
                self.save();
                cx.notify();
            }
            Some(folder) => {
                self.data.settings.global = false;
                self.remember_project(&folder);
                self.resolve_target(None, Some(folder), cx);
            }
        }
    }

    /// Saved projects, most recently used first.
    pub fn recent_projects(&self) -> Vec<PathBuf> {
        let mut projects: Vec<_> = self.data.projects.iter().collect();
        projects.sort_by_key(|p| std::cmp::Reverse(p.last_used));
        projects.into_iter().map(|p| p.path.clone()).collect()
    }

    /// Asks for a folder and makes it the target.
    pub fn add_project(&mut self, cx: &mut Context<Self>) {
        cx.activate(true);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add project".into()),
        });
        self._share = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| this.choose_target(Some(folder), cx))
                .ok();
            cx.update(crate::windows::show_popup);
        }));
    }

    /// Forgets a project (not its files, records, or history entries).
    pub fn forget_project(&mut self, folder: &std::path::Path, cx: &mut Context<Self>) {
        self.data.projects.retain(|p| p.path != folder);
        if self.target.root.as_deref() == Some(folder) {
            self.target = Target::default();
        }
        self.save();
        cx.notify();
    }

    /// Detects the frontmost app's project, or uses `forced`; off the main thread.
    fn resolve_target(
        &mut self,
        app: Option<FrontApp>,
        forced: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let fallback = self.last_project();
        let chosen = forced.is_some();
        // A new palette starts without the last one's notice or waiting equip.
        self.safety_notice = None;
        self.awaiting = None;
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
                let detection = match &forced {
                    Some(folder) => Detection::Folder {
                        folder: folder.clone(),
                        document: None,
                    },
                    None => app
                        .as_ref()
                        .map_or(Detection::Unsupported, frontmost::detect),
                };
                let (root, document) = match (&detection, &forced) {
                    // A picked folder is used as it is.
                    (_, Some(folder)) => (Some(folder.clone()), None),
                    (Detection::Folder { folder, document }, None) => {
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
                this.target.chosen = chosen;
                cx.notify();
            })
            .ok();
        }));
    }

    fn apply_detection(&mut self, app: Option<&FrontApp>, resolved: Resolved) {
        let app_name = app.map_or_else(String::new, |app| app.name.clone());
        if !self.installing {
            // Clears "Finding the project…" from a Return pressed during detection.
            self.install_status = None;
        }
        self.target = Target {
            root: resolved.root,
            stack: resolved.stack,
            installed: resolved.installed,
            signals: resolved.signals,
            ..Default::default()
        };
        match resolved.detection {
            Detection::Folder { .. } => {
                self.target.from_app = app.map(|_| app_name.clone());
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
        // A pasted loadout link or file contents.
        if text.starts_with("loadout:") || text.starts_with('{') {
            self.offer_import(&text, cx);
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
            self.refresh_badge(cx);
            cx.notify();
        }
    }

    /// "Next.js, Tailwind" for the Into line.
    pub fn stack_summary(&self) -> String {
        stack::summary(&self.target.stack, 2)
    }
}

/// Lists each skill's folder (fetching listings `trees` lacks) and reads its text files.
/// Returns a result per favorite, plus listings fetched along the way.
fn scan_skills(
    favorites: Vec<Favorite>,
    mut trees: HashMap<Repo, Tree>,
) -> (Vec<(Uuid, Scan)>, HashMap<Repo, Tree>) {
    let unavailable = |why: &str| Scan::Unavailable(why.to_string().into());
    let mut results = Vec::new();
    for favorite in favorites {
        let Some(skill_ref) = updates::skill_ref(&favorite) else {
            continue;
        };
        let repo = skill_ref.repo.clone();
        if !trees.contains_key(&repo) {
            match updates::fetch_tree(&repo, None) {
                Ok(tree) => {
                    trees.insert(repo.clone(), tree);
                }
                Err(err) => {
                    let why = match err {
                        updates::UpdateError::RateLimited => err.to_string(),
                        _ => "Couldn't load the skill. Check your connection.".into(),
                    };
                    results.push((favorite.id, unavailable(&why)));
                    continue;
                }
            }
        }
        let tree = &trees[&repo];
        let Some(dir) = updates::skill_dir(&favorite, &skill_ref, tree).map(str::to_string) else {
            results.push((
                favorite.id,
                unavailable("Couldn't find this skill in its repo."),
            ));
            continue;
        };
        let files: Vec<(String, String)> = updates::skill_files(tree, &dir)
            .into_iter()
            .filter_map(|path| {
                let full = if dir.is_empty() {
                    path.clone()
                } else {
                    format!("{dir}/{path}")
                };
                updates::fetch_raw(&repo, &full)
                    .ok()
                    .map(|text| (path, text))
            })
            .collect();
        let scan = if files.is_empty() {
            unavailable("Couldn't load the skill. Check your connection.")
        } else {
            let report = safety::scan(&files);
            Scan::Ready(Arc::new(SkillScan { files, report }))
        };
        results.push((favorite.id, scan));
    }
    (results, trees)
}

/// "Web app!" → "web-app", for file names.
fn slug(name: &str) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    if words.is_empty() {
        "loadout".into()
    } else {
        words.join("-")
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn project_name(root: &std::path::Path) -> String {
    root.file_name().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// What a batch of jobs did, by skill name.
#[derive(Default)]
struct Outcome {
    /// Loadouts with at least one skill installed or updated, and where.
    applied: Vec<(Uuid, Option<PathBuf>)>,
    installed: Vec<String>,
    updated: Vec<String>,
    failed: Vec<(String, JobKind)>,
    npx_missing: bool,
}

impl Outcome {
    /// "Installed 2 skills in shop-dashboard", "Updated tdd", "Couldn't update tdd. See log.", …
    fn summary(&self, place: Option<&str>) -> String {
        if self.npx_missing {
            return InstallError::NpxMissing.to_string();
        }
        let skills = |names: &[String]| match names {
            [one] => one.clone(),
            many => format!("{} skills", many.len()),
        };
        let mut done = match (self.installed.is_empty(), self.updated.is_empty()) {
            (false, true) => format!("Installed {}", skills(&self.installed)),
            (true, false) => format!("Updated {}", skills(&self.updated)),
            (false, false) => format!(
                "Installed {} and updated {}",
                skills(&self.installed),
                skills(&self.updated)
            ),
            (true, true) => String::new(),
        };
        if let Some(place) = place.filter(|_| !done.is_empty()) {
            done += &format!(" in {place}");
        }
        if self.failed.is_empty() {
            return done;
        }
        let verb = if self.failed.iter().all(|(_, k)| *k == JobKind::Update) {
            "update"
        } else {
            "install"
        };
        let names: Vec<String> = self.failed.iter().map(|(n, _)| n.clone()).collect();
        let failed = format!("couldn't {verb} {}. See log.", skills(&names));
        if done.is_empty() {
            let mut chars = failed.chars();
            chars
                .next()
                .map_or(failed.clone(), |c| c.to_uppercase().chain(chars).collect())
        } else {
            format!("{done}; {failed}")
        }
    }
}
