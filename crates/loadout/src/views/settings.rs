use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::*;
use loadout_core::{agents, digest};

use crate::macos::login_item::LoginItem;
use crate::state::AppState;

/// Every setting, saved as soon as it changes (text fields on Return or when they lose focus).
pub struct SettingsView {
    state: Entity<AppState>,
    hotkey: Entity<InputState>,
    agents: Entity<InputState>,
    token: Entity<InputState>,
    hotkey_error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = state.read(cx).data.settings.clone();
        let hotkey = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("ctrl+alt+L")
                .default_value(settings.hotkey.clone())
        });
        let agent_list = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Let the skills CLI detect agents")
                .default_value(settings.agents.clone())
        });
        let token = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("ghp_… (optional)")
                .masked(true)
                .default_value(state.read(cx).token.clone().unwrap_or_default())
        });
        let _subscriptions = vec![
            cx.subscribe(&token, |this, input, event: &InputEvent, cx| {
                // Only Return saves: a half-pasted token shouldn't reach the Keychain on blur.
                if let InputEvent::PressEnter { .. } = event {
                    let text = input.read(cx).value().to_string();
                    this.state
                        .update(cx, |state, cx| state.set_token(&text, cx));
                }
            }),
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe(&hotkey, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    let spec = input.read(cx).value().to_string();
                    let ok = this
                        .state
                        .update(cx, |state, cx| state.set_hotkey(&spec, cx));
                    this.hotkey_error = (!ok).then(|| {
                        "Can't use that shortcut. Check the spelling, or another app may own it."
                            .into()
                    });
                    cx.notify();
                }
            }),
            cx.subscribe(&agent_list, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    let text = input.read(cx).value().to_string();
                    this.state
                        .update(cx, |state, cx| state.set_agents(&text, cx));
                }
            }),
        ];
        Self {
            state,
            hotkey,
            agents: agent_list,
            token,
            hotkey_error: None,
            _subscriptions,
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let state = self.state.read(cx);
        let settings = &state.data.settings;
        let digest_now = settings
            .digest_enabled
            .then_some(settings.digest_interval_hours);
        let agent_names: Vec<String> = agents::parse_list(&settings.agents)
            .into_iter()
            .map(|id| agents::find(&id).map_or(id.clone(), |a| a.name.to_string()))
            .collect();
        let agents_hint: SharedString = if agent_names.is_empty() {
            "The skills CLI picks the agents it finds.".into()
        } else {
            format!("Installs for {}.", agent_names.join(", ")).into()
        };
        let has_token = state.token.is_some();
        let (auto_check, launch_after, login_on) = (
            settings.auto_check,
            settings.launch_after_install,
            state.login_item != LoginItem::Off,
        );
        let label = |text: &'static str| {
            div()
                .pt_2()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(muted)
                .child(text)
        };
        let hint = |text: SharedString| div().text_size(px(11.)).text_color(muted).child(text);
        let digest_button = |id: &'static str, text: &'static str, interval: Option<u32>| {
            let state = self.state.clone();
            let button = Button::new(id)
                .xsmall()
                .label(text)
                .on_click(move |_, _, cx| state.update(cx, |s, cx| s.set_digest(interval, cx)));
            if digest_now == interval {
                button.primary()
            } else {
                button.ghost()
            }
        };
        let switch = |id: &'static str,
                      text: &'static str,
                      on: bool,
                      set: fn(&mut AppState, bool, &mut Context<AppState>)| {
            let state = self.state.clone();
            Switch::new(id)
                .checked(on)
                .label(text)
                .small()
                .on_click(move |on, _, cx| state.update(cx, |s, cx| set(s, *on, cx)))
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(label("Palette shortcut"))
            .child(Input::new(&self.hotkey).small())
            .child(match &self.hotkey_error {
                Some(error) => div()
                    .text_size(px(11.))
                    .text_color(theme.danger)
                    .child(error.clone()),
                None => hint("Like ctrl+alt+L or cmd+shift+Space. Return applies it.".into()),
            })
            .child(label("Agents"))
            .child(Input::new(&self.agents).small())
            .child(hint(agents_hint))
            .child(label("GitHub token"))
            .child(Input::new(&self.token).small().mask_toggle())
            .child(hint(
                if has_token {
                    "Saved in your Keychain. GitHub allows 5,000 requests an hour with it."
                } else {
                    "Optional. Without one, GitHub allows 60 requests an hour. Return saves it in your Keychain."
                }
                .into(),
            ))
            .child(label("Update digest"))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(digest_button("digest-off", "Off", None))
                    .child(digest_button("digest-daily", "Daily", Some(digest::DAILY)))
                    .child(digest_button(
                        "digest-weekly",
                        "Weekly",
                        Some(digest::WEEKLY),
                    )),
            )
            .child(hint(
                if digest_now.is_some() {
                    "One quiet summary when updates are pending; no other update notifications."
                } else {
                    "Off: each new version gets its own notification."
                }
                .into(),
            ))
            .child(label("Behavior"))
            .child(switch(
                "auto-check",
                "Check for updates automatically",
                auto_check,
                AppState::set_auto_check,
            ))
            .child(switch(
                "launch-after",
                "Open agent after installing",
                launch_after,
                AppState::set_launch_after_install,
            ))
            .child(switch(
                "login",
                "Open at login",
                login_on,
                AppState::set_open_at_login,
            ))
            .child(
                div().pt_2().child(
                    Button::new("test-notification")
                        .ghost()
                        .xsmall()
                        .label("Send a test notification")
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| state.update(cx, |s, cx| s.send_test_notification(cx))
                        }),
                ),
            )
    }
}
