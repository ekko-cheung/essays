mod dialogs;
mod markdown_edit;
mod panes;
mod theme;

use crate::{
    service::{Result, Service},
    settings::{Appearance, View},
    updates,
};
use essays_core::{
    note::{Note, NoteSummary},
    notebook::Notebook,
    query::NoteQuery,
    status::Status,
    tag::Tag,
};
use gpui_kit::{
    component::{
        ActiveTheme, StyledExt, WindowExt,
        button::{Button, ButtonVariants},
        input::{EditorState, InputEvent, InputState},
        notification::Notification,
        resizable::{ResizableState, h_resizable, resizable_panel},
    },
    prelude::FluentBuilder,
    *,
};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::runtime::Handle;

actions!(
    essays,
    [
        NewNote,
        FindNotes,
        SaveNote,
        SourceView,
        SplitView,
        PreviewView,
        OpenSettings,
        Help,
        Quit,
        NextNote,
        PreviousNote
    ]
);

pub fn bind_keys(cx: &mut App) {
    macro_rules! shortcut {
        ($key:literal) => {
            if cfg!(target_os = "macos") {
                concat!("cmd-", $key)
            } else {
                concat!("ctrl-", $key)
            }
        };
    }
    cx.bind_keys([
        KeyBinding::new(shortcut!("n"), NewNote, Some("Essays")),
        KeyBinding::new(shortcut!("f"), FindNotes, Some("Essays")),
        KeyBinding::new(shortcut!("s"), SaveNote, Some("Essays")),
        KeyBinding::new(shortcut!("2"), SourceView, Some("Essays")),
        KeyBinding::new(shortcut!("3"), SplitView, Some("Essays")),
        KeyBinding::new(shortcut!("4"), PreviewView, Some("Essays")),
        KeyBinding::new(shortcut!(","), OpenSettings, Some("Essays")),
        KeyBinding::new(shortcut!("/"), Help, Some("Essays")),
        KeyBinding::new(shortcut!("q"), Quit, None),
        KeyBinding::new("down", NextNote, Some("EssaysNoteList")),
        KeyBinding::new("up", PreviousNote, Some("EssaysNoteList")),
    ]);
    markdown_edit::register_brackets(cx);
}

pub struct Workspace {
    service: Arc<Service>,
    runtime: Handle,
    query: NoteQuery,
    notebooks: Vec<Notebook>,
    notes: Vec<NoteSummary>,
    statuses: Vec<Status>,
    tags: Vec<Tag>,
    collapsed: HashSet<String>,
    drop_target: Option<(String, panes::DropPosition)>,
    search: Entity<InputState>,
    search_task: Option<Task<()>>,
    workspace_focus: FocusHandle,
    notes_focus: FocusHandle,
    notes_scroll: UniformListScrollHandle,
    title: Entity<InputState>,
    editor: Entity<EditorState>,
    note: Option<Note>,
    save_task: Option<Task<()>>,
    save_failed: bool,
    panels: Entity<ResizableState>,
    syncing: bool,
    checking_updates: bool,
    update: Option<semver::Version>,
    message: Option<(bool, SharedString)>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(
        service: Arc<Service>,
        runtime: Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings = service.settings();
        apply_appearance(settings.appearance, window, cx);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search notes"));
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Untitled"));
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .soft_wrap(true)
                .line_number(false)
                .auto_close(settings.auto_close_brackets)
        });
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.search_changed(cx);
                }
            }),
            cx.subscribe_in(&title, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.edited(cx);
                }
            }),
            cx.subscribe_in(&editor, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.edited(cx);
                }
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                apply_appearance(this.service.settings().appearance, window, cx);
            }),
        ];
        let mut this = Self {
            service,
            runtime,
            query: NoteQuery {
                include_descendants: settings.include_descendants,
                ..Default::default()
            },
            notebooks: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            statuses: Vec::new(),
            collapsed: HashSet::new(),
            drop_target: None,
            search,
            search_task: None,
            workspace_focus: cx.focus_handle(),
            notes_focus: cx.focus_handle(),
            notes_scroll: UniformListScrollHandle::new(),
            title,
            editor,
            note: None,
            save_task: None,
            save_failed: false,
            panels: cx.new(|_| ResizableState::default()),
            syncing: false,
            checking_updates: false,
            update: None,
            message: None,
            _subscriptions: subscriptions,
        };
        this.refresh(window, cx);
        this.search
            .update(cx, |search, cx| search.focus(window, cx));
        if settings.auto_check_updates
            && !cfg!(debug_assertions)
            && !cx.global::<UpdateSession>().checked
        {
            cx.global_mut::<UpdateSession>().checked = true;
            let timer = cx.background_executor().timer(Duration::from_secs(3));
            cx.spawn_in(window, async move |view, cx| {
                timer.await;
                let _ =
                    view.update_in(cx, |view, window, cx| view.check_updates(false, window, cx));
            })
            .detach();
        }
        this
    }

    pub fn report(&mut self, error: String, cx: &mut Context<Self>) {
        self.message = Some((true, error.into()));
        cx.notify();
    }

    fn notice(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.message = Some((false, message.into()));
        cx.notify();
    }

    fn loaded_note(&mut self, note: Option<Note>, window: &mut Window, cx: &mut Context<Self>) {
        self.save_task = None;
        self.save_failed = false;
        self.title.update(cx, |input, cx| {
            input.set_value(note.as_ref().map_or("", |n| n.title.as_str()), window, cx);
        });
        self.editor.update(cx, |editor, cx| {
            editor.set_value(note.as_ref().map_or("", |n| n.body_md.as_str()), window, cx);
        });
        if note.is_none() && self.note.is_some() {
            window.focus(&self.notes_focus, cx);
        }
        self.note = note;
        cx.notify();
    }

    fn search_changed(&mut self, cx: &mut Context<Self>) {
        let timer = cx.background_executor().timer(Duration::from_millis(180));
        self.search_task = Some(cx.spawn(async move |view, cx| {
            timer.await;
            let Ok((service, query)) = view.update(cx, |view, cx| {
                view.query.search = view.search.read(cx).value().to_string();
                (view.service.clone(), view.query.clone())
            }) else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move { service.db(|db| db.query_notes(&query)) })
                .await;
            let _ = view.update(cx, |view, cx| view.set_notes(result, cx));
        }));
    }

    fn set_notes(&mut self, result: Result<Vec<NoteSummary>>, cx: &mut Context<Self>) {
        match result {
            Ok(notes) => self.notes = notes,
            Err(error) => self.report(error, cx),
        }
        cx.notify();
    }

    fn refresh_notes(&mut self, cx: &mut Context<Self>) {
        self.search_task = None;
        self.query.search = self.search.read(cx).value().to_string();
        self.set_notes(self.service.db(|db| db.query_notes(&self.query)), cx);
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let data = self
            .service
            .db(|db| Ok((db.list_notebooks()?, db.list_statuses()?, db.list_tags()?)));
        match data {
            Ok((notebooks, statuses, tags)) => {
                self.notebooks = notebooks;
                self.statuses = statuses;
                self.tags = tags;
            }
            Err(error) => return self.report(error, cx),
        }
        if self
            .query
            .notebook_id
            .as_ref()
            .is_some_and(|id| !self.notebooks.iter().any(|n| &n.id == id))
        {
            self.query.notebook_id = None;
        }
        self.query
            .status_ids
            .retain(|id| self.statuses.iter().any(|s| &s.id == id));
        self.query
            .tag_ids
            .retain(|id| self.tags.iter().any(|t| &t.id == id));
        self.refresh_notes(cx);
        self.refresh_open_note(window, cx);
    }

    fn refresh_open_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(note) = self.note.as_ref() else {
            return;
        };
        let dirty = self.is_dirty(cx);
        let current = self.service.db(|db| match db.get_note(&note.id) {
            Ok(note) => Ok(Some(note)),
            Err(essays_core::Error::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        });
        match current {
            Ok(Some(current))
                if !dirty && (current.title != note.title || current.body_md != note.body_md) =>
            {
                self.loaded_note(Some(current), window, cx);
            }
            Ok(Some(mut current)) => {
                if dirty {
                    current.title.clone_from(&note.title);
                    current.body_md.clone_from(&note.body_md);
                }
                self.note = Some(current);
                cx.notify();
            }
            Ok(None) if !dirty => self.loaded_note(None, window, cx),
            Ok(None) => self.report(
                "This note was deleted during synchronization. Copy your draft before closing it."
                    .into(),
                cx,
            ),
            Err(error) => self.report(error, cx),
        }
    }

    pub fn refresh_after_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh(window, cx);
    }

    fn mutate(
        &mut self,
        operation: impl FnOnce(&essays_core::Db) -> essays_core::Result<()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match self.service.db(operation) {
            Ok(()) => {
                self.refresh(window, cx);
                true
            }
            Err(error) => {
                self.report(error, cx);
                false
            }
        }
    }

    fn new_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(notebook) = self.query.notebook_id.clone() else {
            self.report("Select a notebook to create a note in it.".into(), cx);
            return;
        };
        if !self.flush(cx) {
            return;
        }
        match self.service.db(|db| db.create_note(&notebook)) {
            Ok(note) => {
                self.query.status_ids.clear();
                self.query.tag_ids.clear();
                self.query.no_status = false;
                self.search
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.loaded_note(Some(note), window, cx);
                self.refresh(window, cx);
                if self.service.settings().view == View::Preview {
                    self.set_mode(View::Split, window, cx);
                }
                self.title.update(cx, |input, cx| input.focus(window, cx));
            }
            Err(error) => self.report(error, cx),
        }
    }

    fn select_notebook(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.query.notebook_id == id || !self.flush(cx) {
            return;
        }
        self.query.notebook_id = id;
        self.loaded_note(None, window, cx);
        self.refresh_notes(cx);
    }

    fn select_note(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.note.as_ref().is_some_and(|note| note.id == id) || !self.flush(cx) {
            return;
        }
        match self.service.db(|db| db.get_note(id)) {
            Ok(note) => self.loaded_note(Some(note), window, cx),
            Err(error) => self.report(error, cx),
        }
    }

    fn step_note(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self
            .note
            .as_ref()
            .and_then(|note| self.notes.iter().position(|n| n.id == note.id));
        let index = if next {
            selected.map_or(0, |index| index + 1)
        } else {
            selected.map_or(0, |index| index.saturating_sub(1))
        };
        if let Some(id) = self.notes.get(index).map(|note| note.id.clone()) {
            self.select_note(&id, window, cx);
            self.notes_scroll.scroll_to_item(index, ScrollStrategy::Top);
        }
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.note.as_ref().is_some_and(|note| {
            self.title.read(cx).value().as_ref() != note.title
                || self.editor.read(cx).text().slice(..) != note.body_md.as_str()
        })
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        let timer = cx.background_executor().timer(Duration::from_millis(600));
        self.save_task = Some(cx.spawn(async move |view, cx| {
            timer.await;
            let _ = view.update(cx, |view, cx| {
                view.save_draft(cx);
            });
        }));
        cx.notify();
    }

    pub fn flush(&mut self, cx: &mut Context<Self>) -> bool {
        self.save_task = None;
        self.save_draft(cx)
    }

    fn save_draft(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(note) = self.note.as_mut() else {
            return true;
        };
        let title = self.title.read(cx).value();
        let body = self.editor.read(cx).value();
        if title.as_ref() == note.title && body.as_ref() == note.body_md {
            if self.save_failed {
                self.message = None;
            }
            self.save_failed = false;
            cx.notify();
            return true;
        }
        match self
            .service
            .db(|db| db.update_note(&note.id, &title, &body))
        {
            Ok(at) => {
                note.title = title.to_string();
                note.body_md = body.to_string();
                note.updated_at = at;
                if self.save_failed {
                    self.message = None;
                }
                self.save_failed = false;
                self.refresh_notes(cx);
                true
            }
            Err(error) => {
                self.save_failed = true;
                self.report(format!("Couldn’t save the note. {error}"), cx);
                false
            }
        }
    }

    fn set_mode(&mut self, mode: View, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.service.save_settings(|s| s.view = mode) {
            self.report(error, cx);
            return;
        }
        if self.note.is_some() {
            if mode == View::Preview {
                window.focus(&self.workspace_focus, cx);
            } else {
                self.editor
                    .update(cx, |editor, cx| editor.focus(window, cx));
            }
        }
        cx.notify();
    }

    fn sync_now(&mut self, cx: &mut Context<Self>) {
        if self.syncing || !self.flush(cx) {
            return;
        }
        self.syncing = true;
        self.notice("Syncing…", cx);
        let service = self.service.clone();
        let run = self.runtime.spawn(async move { service.sync().await });
        cx.spawn(async move |view, cx| {
            let result = run
                .await
                .map_err(|e| e.to_string())
                .and_then(|result| result);
            cx.update(|cx| {
                let _ = view.update(cx, |view, cx| {
                    view.syncing = false;
                    match result {
                        Ok(report) => view.notice(
                            format!(
                                "Synced · {} downloaded · {} uploaded",
                                report.pulled, report.pushed
                            ),
                            cx,
                        ),
                        Err(error) => view.report(format!("Sync failed. {error}"), cx),
                    }
                });
                crate::desktop::refresh_open_workspace(cx);
            });
        })
        .detach();
    }

    fn check_updates(&mut self, manual: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.checking_updates {
            return;
        }
        let settings = self.service.settings();
        if !manual && !settings.auto_check_updates {
            return;
        }
        self.checking_updates = true;
        cx.notify();
        let check = self.runtime.spawn(updates::check());
        cx.spawn_in(window, async move |view, cx| {
            let result = check
                .await
                .map_err(|e| e.to_string())
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                view.checking_updates = false;
                match result {
                    Ok(Some(version)) => {
                        let was_dismissed = view.service.settings().dismissed_version.as_deref()
                            == Some(&version.to_string());
                        view.update = Some(version);
                        if manual || !was_dismissed {
                            view.update_dialog(window, cx);
                        }
                    }
                    Ok(None) if manual => {
                        window.push_notification(Notification::info("Essays is up to date."), cx);
                    }
                    Err(error) if manual => {
                        window.push_notification(
                            Notification::error(format!("Couldn’t check for updates. {error}")),
                            cx,
                        );
                    }
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }
}

pub(crate) struct UpdateSession {
    pub checked: bool,
}
impl Global for UpdateSession {}

use theme::apply_appearance;

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = self.service.settings();
        let service = self.service.clone();
        div()
            .id("workspace")
            .key_context("Essays")
            .track_focus(&self.workspace_focus)
            .v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &NewNote, window, cx| this.new_note(window, cx)))
            .on_action(cx.listener(|this, _: &FindNotes, window, cx| {
                this.search.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
            }))
            .on_action(cx.listener(|this, _: &SaveNote, _, cx| {
                this.flush(cx);
            }))
            .on_action(cx.listener(|this, _: &SourceView, window, cx| {
                this.set_mode(View::Source, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &SplitView, window, cx| {
                    this.set_mode(View::Split, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &PreviewView, window, cx| {
                this.set_mode(View::Preview, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &OpenSettings, window, cx| this.settings_dialog(window, cx)),
            )
            .on_action(cx.listener(|this, _: &Help, window, cx| this.help_dialog(window, cx)))
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    h_resizable("workspace-panes")
                        .with_state(&self.panels)
                        .on_resize(move |state, _, cx| {
                            if let [sidebar, list, ..] = &state.read(cx).sizes()[..] {
                                let (sidebar, list) = (f32::from(*sidebar), f32::from(*list));
                                if let Err(error) = service.save_settings(|s| {
                                    s.sidebar_width = Some(sidebar);
                                    s.list_width = Some(list);
                                }) {
                                    eprintln!("Couldn’t save pane sizes: {error}");
                                }
                            }
                        })
                        .child(
                            resizable_panel()
                                .size(px(settings.sidebar_width.unwrap_or(218.)))
                                .size_range(px(150.)..px(420.))
                                .child(self.sidebar(cx)),
                        )
                        .child(
                            resizable_panel()
                                .size(px(settings.list_width.unwrap_or(260.)))
                                .size_range(px(180.)..px(500.))
                                .child(self.note_list(cx)),
                        )
                        .child(
                            resizable_panel()
                                .size_range(px(300.)..px(f32::MAX))
                                .child(self.note_editor(settings.view, cx)),
                        ),
                ),
            )
            .when_some(self.message.clone(), |root, (error, message)| {
                let color = if error {
                    cx.theme().danger
                } else {
                    cx.theme().muted_foreground
                };
                root.child(
                    div()
                        .h_flex()
                        .justify_between()
                        .px_3()
                        .py_1()
                        .gap_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .text_sm()
                        .text_color(color)
                        .child(message)
                        .child(
                            Button::new("dismiss-message")
                                .ghost()
                                .label("Dismiss")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.message = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
    }
}
