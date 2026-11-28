use super::{panes::note_title, *};
use crate::{
    cloud, desktop,
    settings::{CloseAction, Settings, SettingsInput},
};
use gpui_kit::component::{
    Disableable, Selectable, Sizable, WindowExt,
    checkbox::Checkbox,
    dialog::{Confirm, DialogButtonProps},
    form::{Field, Form},
    input::Input,
    notification::Notification,
};

#[derive(Clone, Copy)]
pub(super) enum Metadata {
    Status,
    Tag,
}

impl Metadata {
    fn singular(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Tag => "tag",
        }
    }
    fn plural(self) -> &'static str {
        match self {
            Self::Status => "Statuses",
            Self::Tag => "Tags",
        }
    }

    fn save(
        self,
        db: &essays_core::Db,
        item: Option<&str>,
        name: &str,
        color: &str,
        note: Option<&str>,
    ) -> essays_core::Result<()> {
        match (self, item) {
            (Self::Status, Some(id)) => db.update_status(id, name, color),
            (Self::Tag, Some(id)) => db.update_tag(id, name, color),
            (Self::Status, None) => db.create_status(name, color).map(drop),
            (Self::Tag, None) => match note {
                Some(note) => db.create_tag_for_note(note, name, color).map(drop),
                None => db.create_tag(name, color).map(drop),
            },
        }
    }
}

impl Workspace {
    pub(super) fn notebook_dialog(
        &mut self,
        parent: Option<String>,
        notebook: Option<Notebook>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let heading = if notebook.is_some() {
            "Rename notebook"
        } else if parent.is_some() {
            "New sub-notebook"
        } else {
            "New notebook"
        };
        let name = notebook.as_ref().map_or(String::new(), |n| n.name.clone());
        let owner = cx.entity().downgrade();
        open_name_form(
            heading.into(),
            &name,
            None,
            window,
            cx,
            move |name, _, window, cx| {
                owner
                    .update(cx, |view, cx| {
                        let result = view.service.db(|db| {
                            if let Some(notebook) = &notebook {
                                db.rename_notebook(&notebook.id, name)?;
                                Ok(None)
                            } else {
                                db.create_notebook(parent.as_deref(), name).map(Some)
                            }
                        })?;
                        if let Some(new) = result {
                            if let Some(parent) = &new.parent_id {
                                view.collapsed.remove(parent);
                            }
                            view.refresh(window, cx);
                            view.select_notebook(Some(new.id), window, cx);
                        } else {
                            view.refresh(window, cx);
                        }
                        Ok(())
                    })
                    .map_err(|e| e.to_string())?
            },
        );
    }

    pub(super) fn delete_notebook(
        &mut self,
        notebook: Notebook,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        confirm_delete(
            format!("Delete “{}”?", notebook.name),
            "This also deletes its sub-notebooks and their notes.",
            window,
            cx,
            move |window, cx| {
                owner
                    .update(cx, |view, cx| {
                        if !view.flush(cx) {
                            return Err("Save the open note before deleting this notebook.".into());
                        }
                        view.service.db(|db| db.delete_notebook(&notebook.id))?;
                        view.refresh(window, cx);
                        Ok(())
                    })
                    .map_err(|e| e.to_string())?
            },
        );
    }

    pub(super) fn delete_note(
        &mut self,
        note: NoteSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        confirm_delete(
            format!("Delete “{}”?", note_title(&note.title)),
            "This deletes the note from this device and synced devices.",
            window,
            cx,
            move |window, cx| {
                owner
                    .update(cx, |view, cx| {
                        view.service.db(|db| db.delete_note(&note.id))?;
                        if view.note.as_ref().is_some_and(|open| open.id == note.id) {
                            view.loaded_note(None, window, cx);
                        }
                        view.refresh(window, cx);
                        Ok(())
                    })
                    .map_err(|e| e.to_string())?
            },
        );
    }

    pub(super) fn manage_dialog(
        &mut self,
        kind: Metadata,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(entity) = owner.upgrade() else {
                return dialog;
            };
            let view = entity.read(cx);
            let items: Vec<_> = match kind {
                Metadata::Status => view
                    .statuses
                    .iter()
                    .map(|s| (s.id.clone(), s.name.clone(), s.color.clone(), s.note_count))
                    .collect(),
                Metadata::Tag => view
                    .tags
                    .iter()
                    .map(|t| (t.id.clone(), t.name.clone(), t.color.clone(), t.note_count))
                    .collect(),
            };
            let create = owner.clone();
            dialog
                .title(kind.plural())
                .child(
                    div()
                        .id("metadata-list")
                        .v_flex()
                        .gap_1()
                        .max_h_64()
                        .overflow_y_scroll()
                        .when(items.is_empty(), |list| {
                            list.child(
                                div()
                                    .p_2()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("No {} yet", kind.plural().to_lowercase())),
                            )
                        })
                        .children(items.into_iter().map(|(id, name, color, count)| {
                            let edit = owner.clone();
                            let delete = owner.clone();
                            let edit_item = (id.clone(), name.clone(), color.clone());
                            let removal = (id.clone(), name.clone());
                            let key: SharedString = id.into();
                            div()
                                .h_flex()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .when_some(color_strip(&color), |row, color| {
                                    row.child(div().size_2().rounded_full().bg(color))
                                })
                                .child(div().flex_1().truncate().child(name))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{count}")),
                                )
                                .child(
                                    Button::new(format!("edit-meta-{key}"))
                                        .ghost()
                                        .small()
                                        .label("Edit…")
                                        .on_click(move |_, window, cx| {
                                            let _ = edit.update(cx, |view, cx| {
                                                view.metadata_dialog(
                                                    kind,
                                                    Some(edit_item.clone()),
                                                    None,
                                                    window,
                                                    cx,
                                                )
                                            });
                                        }),
                                )
                                .child(
                                    Button::new(format!("delete-meta-{key}"))
                                        .ghost()
                                        .small()
                                        .label("Delete…")
                                        .on_click(move |_, window, cx| {
                                            let _ = delete.update(cx, |view, cx| {
                                                view.delete_metadata(
                                                    kind,
                                                    removal.clone(),
                                                    window,
                                                    cx,
                                                )
                                            });
                                        }),
                                )
                        })),
                )
                .footer(
                    Button::new("new-meta")
                        .label(format!("New {}…", kind.singular()))
                        .on_click(move |_, window, cx| {
                            let _ = create.update(cx, |view, cx| {
                                view.metadata_dialog(kind, None, None, window, cx)
                            });
                        }),
                )
        });
    }

    pub(super) fn metadata_dialog(
        &mut self,
        kind: Metadata,
        item: Option<(String, String, String)>,
        note: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let heading = format!(
            "{} {}",
            if item.is_some() { "Edit" } else { "New" },
            kind.singular()
        );
        let name = item.as_ref().map_or("", |(_, name, _)| name).to_owned();
        let color = item
            .as_ref()
            .map_or(essays_core::tag::DEFAULT_TAG_COLOR, |(_, _, color)| color)
            .to_owned();
        let owner = cx.entity().downgrade();
        open_name_form(
            heading,
            &name,
            Some(&color),
            window,
            cx,
            move |name, color, window, cx| {
                owner
                    .update(cx, |view, cx| {
                        view.service.db(|db| {
                            kind.save(
                                db,
                                item.as_ref().map(|(id, _, _)| id.as_str()),
                                name,
                                color.unwrap_or(essays_core::tag::DEFAULT_TAG_COLOR),
                                note.as_deref(),
                            )
                        })?;
                        view.refresh(window, cx);
                        Ok(())
                    })
                    .map_err(|e| e.to_string())?
            },
        );
    }

    fn delete_metadata(
        &mut self,
        kind: Metadata,
        (id, name): (String, String),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        confirm_delete(
            format!("Delete “{name}”?"),
            "This removes it from every note that uses it.",
            window,
            cx,
            move |window, cx| {
                owner
                    .update(cx, |view, cx| {
                        view.service.db(|db| match kind {
                            Metadata::Status => db.delete_status(&id),
                            Metadata::Tag => db.delete_tag(&id),
                        })?;
                        view.refresh(window, cx);
                        Ok(())
                    })
                    .map_err(|e| e.to_string())?
            },
        );
    }

    pub(super) fn settings_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        let owner = cx.entity().downgrade();
        let form = cx.new(|cx| {
            SettingsForm::new(
                owner,
                self.service.clone(),
                self.runtime.clone(),
                window,
                cx,
            )
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title("Settings").width(px(535.)).child(form.clone())
        });
    }

    pub(super) fn help_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        let modifier = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl"
        };
        window.open_dialog(cx, move |dialog, _, cx| {
            dialog.title("Keyboard shortcuts").child(
                div().v_flex().gap_2().children(
                    [
                        ("N", "New note"),
                        ("F", "Search notes"),
                        ("S", "Save now"),
                        ("2", "Source view"),
                        ("3", "Split view"),
                        ("4", "Preview"),
                        (",", "Settings"),
                        ("/", "Keyboard shortcuts"),
                        ("Q", "Quit Essays"),
                    ]
                    .map(|(key, label)| {
                        div().h_flex().justify_between().gap_4().child(label).child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{modifier}+{key}")),
                        )
                    }),
                ),
            )
        });
    }

    pub(super) fn update_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(version) = self.update.clone() else {
            return;
        };
        let url = format!("{}/tag/v{version}", updates::RELEASES_URL);
        let service = self.service.clone();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let url = url.clone();
            let service = service.clone();
            let version_string = version.to_string();
            dialog
                .title(format!("Essays {version} is available"))
                .description("Download the installer for your system from GitHub Releases.")
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Open download page")
                        .cancel_text("Later"),
                )
                .on_ok(move |_, _, cx| {
                    cx.open_url(&url);
                    true
                })
                .on_cancel(move |_, window, cx| {
                    if let Err(error) = service
                        .save_settings(|s| s.dismissed_version = Some(version_string.clone()))
                    {
                        window.push_notification(Notification::error(error), cx);
                    }
                    true
                })
        });
    }
}

fn open_name_form(
    heading: String,
    name: &str,
    color: Option<&str>,
    window: &mut Window,
    cx: &mut App,
    submit: impl Fn(&str, Option<&str>, &mut Window, &mut App) -> Result<()> + 'static,
) {
    let name = cx.new(|cx| InputState::new(window, cx).default_value(name));
    let color = color.map(|value| cx.new(|cx| InputState::new(window, cx).default_value(value)));
    let submit = std::rc::Rc::new(submit);
    let focus = name.clone();
    window.open_dialog(cx, move |dialog, _, _| {
        let name_state = name.clone();
        let color_state = color.clone();
        let submit = submit.clone();
        let mut form = Form::new().child(Field::new().label("Name").child(Input::new(&name)));
        if let Some(color) = color.as_ref() {
            form = form.child(Field::new().label("Color").child(Input::new(color)));
        }
        dialog
            .title(heading.clone())
            .overlay_closable(false)
            .on_ok(move |_, window, cx| {
                let name = name_state.read(cx).value();
                let color = color_state.as_ref().map(|state| state.read(cx).value());
                match submit(&name, color.as_deref(), window, cx) {
                    Ok(()) => true,
                    Err(error) => {
                        window.push_notification(Notification::error(error), cx);
                        false
                    }
                }
            })
            .child(form)
            .footer(
                div()
                    .h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(Button::new("save").primary().label("Save").on_click(
                        |_, window, cx| {
                            window.dispatch_action(Box::new(Confirm { secondary: false }), cx);
                        },
                    )),
            )
    });
    focus.update(cx, |input, cx| {
        input.focus(window, cx);
        input.select_all(window, cx);
    });
}

fn confirm_delete(
    title: String,
    description: &'static str,
    window: &mut Window,
    cx: &mut App,
    delete: impl Fn(&mut Window, &mut App) -> Result<()> + 'static,
) {
    let delete = std::rc::Rc::new(delete);
    window.open_alert_dialog(cx, move |dialog, _, _| {
        let delete = delete.clone();
        dialog
            .confirm()
            .title(title.clone())
            .description(description)
            .button_props(
                DialogButtonProps::default()
                    .ok_text("Delete")
                    .cancel_text("Cancel")
                    .ok_variant(gpui_kit::component::button::ButtonVariant::Danger),
            )
            .on_ok(move |_, window, cx| match delete(window, cx) {
                Ok(()) => true,
                Err(error) => {
                    window.push_notification(Notification::error(error), cx);
                    false
                }
            })
    });
}

fn color_strip(color: &str) -> Option<Rgba> {
    u32::from_str_radix(color.strip_prefix('#')?, 16)
        .ok()
        .map(rgb)
}

struct SettingsForm {
    owner: WeakEntity<Workspace>,
    service: Arc<Service>,
    runtime: Handle,
    draft: Settings,
    account: Entity<InputState>,
    database: Entity<InputState>,
    token: Entity<InputState>,
    interval: Entity<InputState>,
    testing: bool,
    test_status: Option<(bool, SharedString)>,
}

impl SettingsForm {
    fn new(
        owner: WeakEntity<Workspace>,
        service: Arc<Service>,
        runtime: Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = service.settings();
        let account =
            cx.new(|cx| InputState::new(window, cx).default_value(draft.sync.account_id.clone()));
        let database =
            cx.new(|cx| InputState::new(window, cx).default_value(draft.sync.database_id.clone()));
        let token = cx.new(|cx| {
            InputState::new(window, cx).masked(true).placeholder(
                if draft.sync.api_token.is_empty() {
                    "API token"
                } else {
                    "Token saved"
                },
            )
        });
        let interval = cx.new(|cx| {
            InputState::new(window, cx).default_value(draft.sync.interval_minutes.to_string())
        });
        Self {
            owner,
            service,
            runtime,
            draft,
            account,
            database,
            token,
            interval,
            testing: false,
            test_status: None,
        }
    }

    fn fields(&self, cx: &App) -> Result<SettingsInput> {
        let interval = self
            .interval
            .read(cx)
            .value()
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=1440).contains(n))
            .ok_or("Enter a sync interval from 1 to 1440 minutes.")?;
        let token = self.token.read(cx).value();
        Ok(SettingsInput {
            close_action: self.draft.close_action,
            auto_close_brackets: self.draft.auto_close_brackets,
            sync_enabled: self.draft.sync.enabled,
            account_id: self.account.read(cx).value().to_string(),
            database_id: self.database.read(cx).value().to_string(),
            api_token: (!token.trim().is_empty()).then(|| token.to_string()),
            auto_sync: self.draft.sync.auto_sync,
            interval_minutes: interval,
        })
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let result = self.fields(cx).and_then(|fields| {
            self.service.save_settings(|s| {
                s.apply(fields);
                s.appearance = self.draft.appearance;
                s.auto_check_updates = self.draft.auto_check_updates;
            })
        });
        if let Err(error) = result {
            self.test_status = Some((true, error.into()));
            cx.notify();
            return false;
        }
        let settings = self.service.settings();
        apply_appearance(settings.appearance, window, cx);
        if let Err(error) = desktop::refresh_tray(cx) {
            window.push_notification(Notification::error(error), cx);
        }
        let _ = self.owner.update(cx, |view, cx| {
            view.editor.update(cx, |editor, cx| {
                editor.set_auto_close(settings.auto_close_brackets, window, cx)
            });
            cx.notify();
        });
        true
    }

    fn test_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.testing {
            return;
        }
        let fields = match self.fields(cx) {
            Ok(fields) => fields,
            Err(error) => {
                self.test_status = Some((true, error.into()));
                cx.notify();
                return;
            }
        };
        let mut settings = self.service.settings();
        settings.apply(fields);
        self.testing = true;
        self.test_status = Some((false, "Testing connection…".into()));
        cx.notify();
        let task = self
            .runtime
            .spawn(async move { cloud::test_connection(&settings.sync).await });
        cx.spawn_in(window, async move |form, cx| {
            let result = task
                .await
                .map_err(|e| e.to_string())
                .and_then(|result| result);
            let _ = form.update_in(cx, |form, _, cx| {
                form.testing = false;
                form.test_status = Some(match result {
                    Ok(()) => (false, "Connected to Cloudflare D1.".into()),
                    Err(error) => (true, format!("Couldn’t connect. {error}").into()),
                });
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for SettingsForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = self.draft.appearance;
        let close = self.draft.close_action;
        let sync_active = self.draft.sync.enabled;
        let (checking, syncing) = self.owner.upgrade().map_or((false, false), |view| {
            let view = view.read(cx);
            (view.checking_updates, view.syncing)
        });
        let sync_status = self.service.sync_status().map_or_else(
            |error| format!("Couldn’t read sync status. {error}"),
            |(pending, last)| {
                let last = last.map_or("Never synced".into(), |time| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    let minutes = (now.saturating_sub(time) / 60_000).max(0);
                    if minutes == 0 {
                        "Synced just now".into()
                    } else {
                        format!("Synced {minutes} min ago")
                    }
                });
                format!("{last} · {pending} pending")
            },
        );
        let max_height = (window.viewport_size().height - px(200.)).max(px(180.));
        div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .id("settings-scroll")
                    .v_flex()
                    .gap_4()
                    .max_h(max_height)
                    .overflow_y_scroll()
                    .pr_2()
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Appearance"))
                            .child(
                                div().h_flex().gap_1().children(
                                    [
                                        ("System", Appearance::System),
                                        ("Light", Appearance::Light),
                                        ("Dark", Appearance::Dark),
                                    ]
                                    .map(|(label, mode)| {
                                        Button::new(label)
                                            .outline()
                                            .small()
                                            .label(label)
                                            .selected(appearance == mode)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.draft.appearance = mode;
                                                cx.notify();
                                            }))
                                    }),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .pt_3()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("General"))
                            .child(
                                Checkbox::new("auto-close")
                                    .label("Complete brackets and code fences")
                                    .checked(self.draft.auto_close_brackets)
                                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                                        this.draft.auto_close_brackets = *value;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Checkbox::new("close-to-tray")
                                    .label("Keep running in the tray when the window closes")
                                    .checked(close == CloseAction::MinimizeToTray)
                                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                                        this.draft.close_action = if *value {
                                            CloseAction::MinimizeToTray
                                        } else {
                                            CloseAction::Quit
                                        };
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .pt_3()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Cloudflare D1 sync"),
                            )
                            .child(
                                Checkbox::new("enable-sync")
                                    .label("Enable sync")
                                    .checked(sync_active)
                                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                                        this.draft.sync.enabled = *value;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Form::new()
                                    .child(
                                        Field::new()
                                            .label("Account ID")
                                            .child(Input::new(&self.account)),
                                    )
                                    .child(
                                        Field::new()
                                            .label("Database ID")
                                            .child(Input::new(&self.database)),
                                    )
                                    .child(
                                        Field::new()
                                            .label("API token")
                                            .child(Input::new(&self.token)),
                                    )
                                    .child(
                                        Field::new()
                                            .label("Interval in minutes")
                                            .child(Input::new(&self.interval)),
                                    ),
                            )
                            .child(
                                Checkbox::new("auto-sync")
                                    .label("Sync automatically")
                                    .checked(self.draft.sync.auto_sync)
                                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                                        this.draft.sync.auto_sync = *value;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(sync_status),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("test-sync")
                                            .outline()
                                            .small()
                                            .label("Test connection")
                                            .disabled(self.testing)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.test_connection(window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("save-sync")
                                            .outline()
                                            .small()
                                            .label("Save and sync")
                                            .disabled(!sync_active || syncing)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                if this.save(window, cx) {
                                                    window.close_dialog(cx);
                                                    let _ = this
                                                        .owner
                                                        .update(cx, |view, cx| view.sync_now(cx));
                                                }
                                            })),
                                    ),
                            )
                            .when_some(self.test_status.clone(), |section, (error, status)| {
                                section.child(
                                    div()
                                        .text_sm()
                                        .text_color(if error {
                                            cx.theme().danger
                                        } else {
                                            cx.theme().muted_foreground
                                        })
                                        .child(status),
                                )
                            }),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .pt_3()
                            .child(
                                div()
                                    .h_flex()
                                    .justify_between()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Updates"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(concat!("Essays ", env!("CARGO_PKG_VERSION"))),
                                    ),
                            )
                            .child(
                                Checkbox::new("auto-updates")
                                    .label("Check for updates at startup")
                                    .checked(self.draft.auto_check_updates)
                                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                                        this.draft.auto_check_updates = *value;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("check-update")
                                    .outline()
                                    .small()
                                    .label("Check for updates")
                                    .disabled(checking)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let _ = this.owner.update(cx, |view, cx| {
                                            view.check_updates(true, window, cx)
                                        });
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .justify_end()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_3()
                    .child(
                        Button::new("cancel-settings")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("save-settings")
                            .primary()
                            .label("Save")
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.save(window, cx) {
                                    window.close_dialog(cx);
                                }
                            })),
                    ),
            )
    }
}
