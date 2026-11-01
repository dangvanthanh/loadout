use std::path::PathBuf;

use gpui_kit::*;

use crate::macos::frontmost::{self, Detection, FrontApp};

/// Where the palette installs.
#[derive(Clone, Debug, Default)]
pub struct Target {
    pub folder: Option<PathBuf>,
    /// The app the folder came from; `None` when falling back to the last project.
    pub from_app: Option<String>,
    /// Why detection fell back, phrased as what to do.
    pub notice: Option<SharedString>,
    pub detecting: bool,
}

#[derive(Default)]
pub struct AppState {
    pub target: Target,
    // shortcut: session-only fallback, persisted as `projects` once storage lands (T1.1).
    last_project: Option<PathBuf>,
    _detect: Option<Task<()>>,
}

pub struct GlobalAppState(pub Entity<AppState>);

impl Global for GlobalAppState {}

impl AppState {
    /// Resolves the target for `app` off the main thread; the last project stands in meanwhile.
    pub fn detect_target(&mut self, app: Option<FrontApp>, cx: &mut Context<Self>) {
        self.target = Target {
            folder: self.last_project.clone(),
            detecting: app.is_some(),
            ..Default::default()
        };
        cx.notify();
        let Some(app) = app else {
            self._detect = None;
            return;
        };
        let task = cx.background_executor().spawn({
            let app = app.clone();
            async move { frontmost::detect(&app) }
        });
        // Replacing the task cancels a detection still running for an earlier palette.
        self._detect = Some(cx.spawn(async move |this, cx| {
            let detection = task.await;
            this.update(cx, |this, cx| {
                this.apply_detection(&app, detection);
                cx.notify();
            })
            .ok();
        }));
    }

    fn apply_detection(&mut self, app: &FrontApp, detection: Detection) {
        self.target.detecting = false;
        match detection {
            Detection::Folder(folder) => {
                self.last_project = Some(folder.clone());
                self.target.folder = Some(folder);
                self.target.from_app = Some(app.name.clone());
            }
            Detection::NeedsAutomation => {
                self.target.notice = Some(
                    "Couldn't read the Finder window. Allow Automation in System Settings.".into(),
                );
            }
            Detection::NeedsAccessibility => {
                self.target.notice = Some(
                    format!(
                        "Couldn't read the {} window. Allow Accessibility in System Settings.",
                        app.name
                    )
                    .into(),
                );
            }
            Detection::Unsupported => {}
        }
    }
}
