use std::path::PathBuf;

use gpui_kit::*;
use loadout_core::model::{Project, SavedState};
use loadout_core::stack::{self, StackTag};
use loadout_core::{project, provenance, storage};

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
}

pub struct AppState {
    pub data: SavedState,
    /// `None` when data.json couldn't be read: never overwrite what we couldn't load.
    data_path: Option<PathBuf>,
    pub target: Target,
    pub login_item: LoginItem,
    /// Last popup status line, e.g. why open at login failed.
    pub popup_notice: Option<SharedString>,
    _detect: Option<Task<()>>,
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
            _detect: None,
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
        let now = Some(provenance::now());
        let id = match self.data.projects.iter_mut().find(|p| p.path == root) {
            Some(project) => {
                project.last_used = now;
                project.id
            }
            None => {
                let project = Project::new(root.to_path_buf(), now);
                let id = project.id;
                self.data.projects.push(project);
                id
            }
        };
        self.data.selected_project_id = Some(id);
        self.save();
    }

    /// Resolves the target for `app` off the main thread; the last project stands in meanwhile.
    pub fn detect_target(&mut self, app: Option<FrontApp>, cx: &mut Context<Self>) {
        let fallback = self.last_project();
        self.target = Target {
            root: fallback.clone(),
            detecting: true,
            ..Default::default()
        };
        cx.notify();
        let task = cx.background_executor().spawn({
            let app = app.clone();
            async move {
                let detection = app
                    .as_ref()
                    .map_or(Detection::Unsupported, frontmost::detect);
                let root = match &detection {
                    Detection::Folder(folder) => Some(project::root_for(folder)),
                    _ => fallback,
                };
                let stack = root.as_deref().map(stack::detect).unwrap_or_default();
                (detection, root, stack)
            }
        });
        // Replacing the task cancels a detection still running for an earlier palette.
        self._detect = Some(cx.spawn(async move |this, cx| {
            let (detection, root, stack) = task.await;
            this.update(cx, |this, cx| {
                this.apply_detection(app.as_ref(), detection, root, stack);
                cx.notify();
            })
            .ok();
        }));
    }

    fn apply_detection(
        &mut self,
        app: Option<&FrontApp>,
        detection: Detection,
        root: Option<PathBuf>,
        stack: Vec<StackTag>,
    ) {
        let app_name = app.map_or_else(String::new, |app| app.name.clone());
        self.target = Target {
            root,
            stack,
            ..Default::default()
        };
        match detection {
            Detection::Folder(_) => {
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

    /// "Next.js, Tailwind" for the Into line.
    pub fn stack_summary(&self) -> String {
        stack::summary(&self.target.stack, 2)
    }
}
