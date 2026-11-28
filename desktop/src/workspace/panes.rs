use super::*;
use gpui_kit::component::{
    Icon, IconName, Selectable, Sizable,
    input::{Editor, Input},
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    tab::TabBar,
    text::TextView,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DropPosition {
    Before,
    Inside,
    After,
}

#[derive(Clone)]
struct NotebookDrag {
    id: String,
    name: String,
}

impl Render for NotebookDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .border_1()
            .rounded(cx.theme().radius)
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.name.clone())
    }
}

impl Workspace {
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let toolbar = div()
            .h_flex()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Notebooks"),
            )
            .child(
                Button::new("new-notebook")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .accessibility_label("New notebook")
                    .tooltip("New notebook…")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.notebook_dialog(None, None, window, cx)
                    })),
            );
        div()
            .v_flex()
            .size_full()
            .min_h_0()
            .p_3()
            .gap_2()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .child(toolbar)
            .child(
                Input::new(&self.search)
                    .small()
                    .aria_label("Search notes")
                    .prefix(
                        Icon::new(IconName::Search)
                            .small()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                div().h_flex().gap_2().child(self.filters(cx)).child(
                    Button::new("manage")
                        .ghost()
                        .small()
                        .label("Manage")
                        .dropdown_menu({
                            let entity = cx.entity().downgrade();
                            move |menu, _, _| {
                                let status = entity.clone();
                                let tag = entity.clone();
                                menu.item(PopupMenuItem::new("Statuses…").on_click(
                                    move |_, window, cx| {
                                        let _ = status.update(cx, |this, cx| {
                                            this.manage_dialog(
                                                super::dialogs::Metadata::Status,
                                                window,
                                                cx,
                                            )
                                        });
                                    },
                                ))
                                .item(
                                    PopupMenuItem::new("Tags…").on_click(move |_, window, cx| {
                                        let _ = tag.update(cx, |this, cx| {
                                            this.manage_dialog(
                                                super::dialogs::Metadata::Tag,
                                                window,
                                                cx,
                                            )
                                        });
                                    }),
                                )
                            }
                        }),
                ),
            )
            .child(
                div()
                    .id("notebook-tree")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .mt_1()
                    .gap_0p5()
                    .overflow_y_scroll()
                    .child(
                        Button::new("all-notes")
                            .ghost()
                            .small()
                            .w_full()
                            .accessibility_label("All notes")
                            .icon(IconName::BookOpen)
                            .selected(self.query.notebook_id.is_none())
                            .when(self.query.notebook_id.is_none(), |button| {
                                button
                                    .bg(cx.theme().sidebar_accent)
                                    .text_color(cx.theme().sidebar_accent_foreground)
                            })
                            .child(div().flex_1().truncate().child("All notes"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select_notebook(None, window, cx)
                            })),
                    )
                    .children(self.notebook_rows(cx)),
            )
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .gap_1()
                    .pt_2()
                    .border_t_1()
                    .border_color(cx.theme().sidebar_border)
                    .child(
                        Button::new("settings")
                            .ghost()
                            .small()
                            .icon(IconName::Settings)
                            .label("Settings…")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.settings_dialog(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("help")
                            .ghost()
                            .small()
                            .icon(IconName::Info)
                            .accessibility_label("Keyboard shortcuts")
                            .tooltip("Keyboard shortcuts…")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.help_dialog(window, cx)),
                            ),
                    ),
            )
    }

    fn filters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.query.clone();
        let statuses = self.statuses.clone();
        let tags = self.tags.clone();
        let owner = cx.entity().downgrade();
        let count = query.status_ids.len() + query.tag_ids.len() + usize::from(query.no_status);
        Button::new("filters")
            .outline()
            .small()
            .label(if count == 0 {
                "Filters".into()
            } else {
                format!("Filters ({count})")
            })
            .dropdown_menu(move |menu, _, _| {
                let view = owner.clone();
                let mut menu = menu
                    .item(
                        PopupMenuItem::new("Include sub-notebooks")
                            .checked(query.include_descendants)
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    let include = !this.query.include_descendants;
                                    match this
                                        .service
                                        .save_settings(|s| s.include_descendants = include)
                                    {
                                        Ok(()) => {
                                            this.query.include_descendants = include;
                                            this.refresh_notes(cx);
                                        }
                                        Err(error) => this.report(error, cx),
                                    }
                                });
                            }),
                    )
                    .separator()
                    .item(PopupMenuItem::label("Status"));
                let view = owner.clone();
                menu = menu.item(
                    PopupMenuItem::new("No status")
                        .checked(query.no_status)
                        .on_click(move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.query.no_status = !this.query.no_status;
                                this.refresh_notes(cx);
                            });
                        }),
                );
                for status in &statuses {
                    let id = status.id.clone();
                    let view = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("{} ({})", status.name, status.note_count))
                            .checked(query.status_ids.contains(&id))
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    toggle_id(&mut this.query.status_ids, &id);
                                    this.refresh_notes(cx);
                                });
                            }),
                    );
                }
                menu = menu.separator().item(PopupMenuItem::label("Tags"));
                for tag in &tags {
                    let id = tag.id.clone();
                    let view = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("{} ({})", tag.name, tag.note_count))
                            .checked(query.tag_ids.contains(&id))
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    toggle_id(&mut this.query.tag_ids, &id);
                                    this.refresh_notes(cx);
                                });
                            }),
                    );
                }
                let view = owner.clone();
                menu.separator().item(
                    PopupMenuItem::new("Clear filters")
                        .disabled(count == 0)
                        .on_click(move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.query.status_ids.clear();
                                this.query.tag_ids.clear();
                                this.query.no_status = false;
                                this.refresh_notes(cx);
                            });
                        }),
                )
            })
    }

    fn notebook_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        use std::collections::HashMap;
        let ids: HashSet<_> = self.notebooks.iter().map(|n| n.id.as_str()).collect();
        let mut children: HashMap<Option<&str>, Vec<&Notebook>> = HashMap::new();
        for item in &self.notebooks {
            let parent = item.parent_id.as_deref().filter(|id| ids.contains(id));
            children.entry(parent).or_default().push(item);
        }
        let mut stack: Vec<_> = children
            .get(&None)
            .into_iter()
            .flatten()
            .rev()
            .map(|node| (*node, 0usize))
            .collect();
        let mut rows = Vec::new();
        while let Some((node, depth)) = stack.pop() {
            let nested = children.get(&Some(node.id.as_str()));
            rows.push(self.notebook_row(node, depth, nested.is_some(), cx));
            if !self.collapsed.contains(&node.id) {
                stack.extend(
                    nested
                        .into_iter()
                        .flatten()
                        .rev()
                        .map(|node| (*node, depth + 1)),
                );
            }
        }
        rows
    }

    fn notebook_row(
        &self,
        notebook: &Notebook,
        depth: usize,
        has_children: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = notebook.id.clone();
        let row_id: SharedString = id.clone().into();
        let selected = self.query.notebook_id.as_ref() == Some(&id);
        let collapsed = self.collapsed.contains(&id);
        let open_id = id.clone();
        let toggle = id.clone();
        let drag_id = id.clone();
        let drop_id = id.clone();
        let target = self
            .drop_target
            .as_ref()
            .filter(|(row, _)| row == &id)
            .map(|(_, position)| *position);
        let accent = cx.theme().ring;
        let notebook = notebook.clone();
        let entity = cx.entity().downgrade();
        let indent = rems(0.75 * depth.min(12) as f32);
        div()
            .id(format!("notebook-row-{row_id}"))
            .h_flex()
            .gap_1()
            .h_8()
            .pl(indent)
            .border_t_1()
            .border_b_1()
            .border_color(gpui_kit::transparent_black())
            .drag_over::<NotebookDrag>(move |style, _, _, _| match target {
                Some(DropPosition::Before) => style.border_b_0().border_color(accent),
                Some(DropPosition::After) => style.border_t_0().border_color(accent),
                _ => style.bg(accent.opacity(0.17)),
            })
            .on_drag(
                NotebookDrag {
                    id: id.clone(),
                    name: notebook.name.clone(),
                },
                |drag, _, _, cx| cx.new(|_| drag.clone()),
            )
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<NotebookDrag>, _, cx| {
                    let fraction =
                        (event.event.position.y - event.bounds.origin.y) / event.bounds.size.height;
                    let position = if fraction < 0.25 {
                        DropPosition::Before
                    } else if fraction > 0.75 {
                        DropPosition::After
                    } else {
                        DropPosition::Inside
                    };
                    if this.drop_target.as_ref() != Some(&(drag_id.clone(), position)) {
                        this.drop_target = Some((drag_id.clone(), position));
                        cx.notify();
                    }
                }),
            )
            .on_drop(cx.listener(move |this, drag: &NotebookDrag, window, cx| {
                let position = this
                    .drop_target
                    .take()
                    .filter(|(id, _)| id == &drop_id)
                    .map_or(DropPosition::Inside, |(_, pos)| pos);
                this.move_notebook(&drag.id, &drop_id, position, window, cx);
            }))
            .child(if has_children {
                Button::new(format!("collapse-{row_id}"))
                    .ghost()
                    .xsmall()
                    .icon(if collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .accessibility_label(if collapsed {
                        "Expand notebook"
                    } else {
                        "Collapse notebook"
                    })
                    .tooltip(if collapsed {
                        "Expand notebook"
                    } else {
                        "Collapse notebook"
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.collapsed.remove(&toggle) {
                            this.collapsed.insert(toggle.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                div().w_5().into_any_element()
            })
            .child(
                Button::new(format!("notebook-{row_id}"))
                    .ghost()
                    .small()
                    .selected(selected)
                    .when(selected, |button| {
                        button
                            .bg(cx.theme().sidebar_accent)
                            .text_color(cx.theme().sidebar_accent_foreground)
                    })
                    .min_w_0()
                    .flex_1()
                    .accessibility_label(notebook.name.clone())
                    .child(div().flex_1().truncate().child(notebook.name.clone()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_notebook(Some(open_id.clone()), window, cx)
                    })),
            )
            .child(
                Button::new(format!("notebook-menu-{row_id}"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Ellipsis)
                    .accessibility_label(format!("Actions for {}", notebook.name))
                    .tooltip("Notebook actions")
                    .dropdown_menu(move |menu, window, cx| {
                        let create = entity.clone();
                        let rename = entity.clone();
                        let delete = entity.clone();
                        let parent = notebook.id.clone();
                        let target = notebook.clone();
                        let removal = notebook.clone();
                        let moving = notebook.clone();
                        let move_owner = entity.clone();
                        menu.item(PopupMenuItem::new("New sub-notebook…").on_click(
                            move |_, window, cx| {
                                let _ = create.update(cx, |this, cx| {
                                    this.notebook_dialog(Some(parent.clone()), None, window, cx)
                                });
                            },
                        ))
                        .item(
                            PopupMenuItem::new("Rename…").on_click(move |_, window, cx| {
                                let _ = rename.update(cx, |this, cx| {
                                    this.notebook_dialog(None, Some(target.clone()), window, cx)
                                });
                            }),
                        )
                        .submenu("Move", window, cx, move |menu, _, cx| {
                            notebook_move_menu(menu, &moving, &move_owner, cx)
                        })
                        .separator()
                        .item(
                            PopupMenuItem::new("Delete…").on_click(move |_, window, cx| {
                                let _ = delete.update(cx, |this, cx| {
                                    this.delete_notebook(removal.clone(), window, cx)
                                });
                            }),
                        )
                    }),
            )
            .into_any_element()
    }

    fn move_notebook(
        &mut self,
        id: &str,
        target: &str,
        position: DropPosition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.notebooks.iter().find(|n| n.id == target && n.id != id) else {
            return;
        };
        let parent = if position == DropPosition::Inside {
            Some(target.id.clone())
        } else {
            target.parent_id.clone()
        };
        let siblings = self
            .notebooks
            .iter()
            .filter(|n| n.parent_id == parent && n.id != id);
        let index = if position == DropPosition::Inside {
            siblings.count()
        } else {
            siblings.take_while(|n| n.id != target.id).count()
                + usize::from(position == DropPosition::After)
        };
        self.reposition_notebook(id, parent, index, window, cx);
    }

    fn reposition_notebook(
        &mut self,
        id: &str,
        parent: Option<String>,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mutate(
            |db| db.move_notebook(id, parent.as_deref(), index),
            window,
            cx,
        ) && let Some(parent) = parent
        {
            self.collapsed.remove(&parent);
        }
    }

    pub(super) fn note_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let heading = self
            .query
            .notebook_id
            .as_ref()
            .and_then(|id| self.notebooks.iter().find(|n| &n.id == id))
            .map_or("All notes", |n| &n.name);
        let count = self.notes.len();
        let count_label = if count == 1 {
            "1 note".to_owned()
        } else {
            format!("{count} notes")
        };
        let ring = cx.theme().ring;
        div()
            .id("notes-pane")
            .v_flex()
            .size_full()
            .min_h_0()
            .bg(cx.theme().colors.list)
            .border_1()
            .border_color(transparent_black())
            .focus_visible(move |style| style.border_color(ring))
            .key_context("EssaysNoteList")
            .track_focus(&self.notes_focus)
            .tab_stop(true)
            .on_action(
                cx.listener(|this, _: &NextNote, window, cx| this.step_note(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PreviousNote, window, cx| this.step_note(false, window, cx)),
            )
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .bg(cx.theme().list_head)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .v_flex()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(heading.to_owned()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(count_label),
                            ),
                    )
                    .child(
                        Button::new("new-note")
                            .outline()
                            .small()
                            .icon(IconName::Plus)
                            .accessibility_label("New note")
                            .tooltip("New note")
                            .on_click(cx.listener(|this, _, window, cx| this.new_note(window, cx))),
                    ),
            )
            .when(self.notes.is_empty(), |pane| {
                pane.child(
                    div()
                        .p_4()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            if self.query.search.is_empty()
                                && self.query.tag_ids.is_empty()
                                && self.query.status_ids.is_empty()
                                && !self.query.no_status
                            {
                                "No notes yet"
                            } else {
                                "No notes match your search or filters"
                            },
                        ),
                )
            })
            .when(!self.notes.is_empty(), |pane| {
                pane.child(
                    uniform_list(
                        "notes",
                        self.notes.len(),
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            this.notes[range]
                                .iter()
                                .map(|note| this.note_row(note, cx))
                                .collect()
                        }),
                    )
                    .track_scroll(&self.notes_scroll)
                    .w_full()
                    .flex_1()
                    .min_h_0(),
                )
            })
    }

    fn note_row(&self, note: &NoteSummary, cx: &Context<Self>) -> AnyElement {
        let id: SharedString = note.id.clone().into();
        let selected = self
            .note
            .as_ref()
            .is_some_and(|current| current.id == note.id);
        let title = note_title(&note.title);
        let snippet = note
            .snippet
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let mut metadata = Vec::new();
        if note.pinned {
            metadata.push("Pinned");
        }
        if let Some(status) = self
            .statuses
            .iter()
            .find(|s| Some(&s.id) == note.status_id.as_ref())
        {
            metadata.push(status.name.as_str());
        }
        metadata.extend(
            self.tags
                .iter()
                .filter(|tag| note.tag_ids.contains(&tag.id))
                .map(|tag| tag.name.as_str()),
        );
        let open = note.id.clone();
        let menu_note = note.clone();
        let entity = cx.entity().downgrade();
        let hover = if selected {
            cx.theme().list_active
        } else {
            cx.theme().list_hover
        };
        let focus = self.notes_focus.clone();
        div()
            .id(format!("note-row-{id}"))
            .h_flex()
            .relative()
            .w_full()
            .h(rems(4.5))
            .gap_2()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .when(selected, |row| {
                row.bg(cx.theme().list_active).child(
                    div()
                        .absolute()
                        .left_0()
                        .top_2()
                        .bottom_2()
                        .w_0p5()
                        .rounded_full()
                        .bg(cx.theme().list_active_border),
                )
            })
            .hover(move |row| row.bg(hover))
            .on_click(cx.listener(move |this, _, window, cx| {
                window.focus(&focus, cx);
                this.select_note(&open, window, cx);
            }))
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title.to_owned()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(snippet),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(metadata.join(" · ")),
                    ),
            )
            .child(
                Button::new(format!("note-menu-{id}"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Ellipsis)
                    .accessibility_label(format!("Actions for {title}"))
                    .tooltip("Note actions")
                    .dropdown_menu(move |menu, _, _| {
                        let pin_owner = entity.clone();
                        let delete_owner = entity.clone();
                        let pin = menu_note.clone();
                        let delete = menu_note.clone();
                        menu.item(
                            PopupMenuItem::new(if pin.pinned { "Unpin" } else { "Pin" }).on_click(
                                move |_, window, cx| {
                                    let _ = pin_owner.update(cx, |this, cx| {
                                        this.mutate(
                                            |db| db.set_note_pinned(&pin.id, !pin.pinned),
                                            window,
                                            cx,
                                        );
                                    });
                                },
                            ),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new("Delete…").on_click(move |_, window, cx| {
                                let _ = delete_owner.update(cx, |this, cx| {
                                    this.delete_note(delete.clone(), window, cx)
                                });
                            }),
                        )
                    }),
            )
            .into_any_element()
    }

    pub(super) fn note_editor(&self, mode: View, cx: &mut Context<Self>) -> AnyElement {
        let Some(note) = self.note.as_ref() else {
            let missing_notebooks = self.notebooks.is_empty();
            let can_create_note = self.query.notebook_id.is_some() && self.notes.is_empty();
            return div()
                .v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .p_4()
                .gap_3()
                .child(
                    div()
                        .h_flex()
                        .justify_center()
                        .size_10()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().sidebar_accent)
                        .child(Icon::new(IconName::FileText).text_color(cx.theme().primary)),
                )
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_center()
                        .child(if missing_notebooks {
                            "Create your first notebook"
                        } else if can_create_note {
                            "Start a note"
                        } else {
                            "Select a note"
                        }),
                )
                .when(missing_notebooks || can_create_note, |pane| {
                    let label = if missing_notebooks {
                        "New notebook…"
                    } else {
                        "New note"
                    };
                    pane.child(
                        Button::new("empty-workspace-action")
                            .outline()
                            .label(label)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if missing_notebooks {
                                    this.notebook_dialog(None, None, window, cx);
                                } else {
                                    this.new_note(window, cx);
                                }
                            })),
                    )
                })
                .into_any_element();
        };
        let (save_label, save_icon, save_color) = if self.save_failed {
            (
                "Couldn’t save",
                Some(IconName::CircleAlert),
                cx.theme().danger,
            )
        } else if self.is_dirty(cx) {
            ("Unsaved", None, cx.theme().foreground)
        } else {
            ("Saved", Some(IconName::Check), cx.theme().muted_foreground)
        };
        let modes = [View::Source, View::Split, View::Preview];
        let mode_index = modes.iter().position(|value| *value == mode).unwrap_or(1);
        let preview = || {
            let markdown = self.editor.read(cx).value();
            div()
                .v_flex()
                .size_full()
                .min_w_0()
                .min_h_0()
                .when(mode == View::Split, |pane| {
                    pane.child(document_pane_heading("Preview", cx))
                })
                .child(
                    div().flex_1().min_h_0().p_4().child(
                        TextView::markdown(format!("preview-{}", note.id), markdown)
                            .scrollable(true)
                            .size_full()
                            .on_link_click(|url, _, _, cx| {
                                if url.starts_with("https://")
                                    || url.starts_with("http://")
                                    || url.starts_with("mailto:")
                                {
                                    cx.open_url(url);
                                }
                            }),
                    ),
                )
        };
        let source = || {
            div()
                .v_flex()
                .size_full()
                .min_w_0()
                .min_h_0()
                .bg(cx.theme().muted)
                .when(mode == View::Split, |pane| {
                    pane.child(document_pane_heading("Markdown", cx))
                })
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if this.service.settings().auto_close_brackets {
                        super::markdown_edit::complete_fence(&this.editor, event, window, cx);
                    }
                }))
                .child(
                    div().flex_1().min_h_0().px_2().pt_2().child(
                        Editor::new(&self.editor)
                            .size_full()
                            .appearance(false)
                            .aria_label("Markdown source"),
                    ),
                )
        };
        let body = match mode {
            View::Source => source().into_any_element(),
            View::Split => h_resizable("markdown-split")
                .child(
                    resizable_panel()
                        .size_range(px(110.)..px(f32::MAX))
                        .child(source()),
                )
                .child(
                    resizable_panel()
                        .size_range(px(110.)..px(f32::MAX))
                        .child(preview()),
                )
                .into_any_element(),
            View::Preview => preview().into_any_element(),
        };
        div()
            .v_flex()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .px_4()
                    .pt_4()
                    .pb_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(if mode == View::Preview {
                        div()
                            .h_flex()
                            .h_8()
                            .px_1()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(note_title(&self.title.read(cx).value()).to_owned())
                            .into_any_element()
                    } else {
                        Input::new(&self.title)
                            .bg(cx.theme().background)
                            .border_color(transparent_black())
                            .px_1()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .aria_label("Note title")
                            .into_any_element()
                    })
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(self.status_menu(note, cx))
                            .child(self.tag_menu(note, cx))
                            .child(div().flex_1())
                            .child(
                                Button::new("save-note")
                                    .ghost()
                                    .small()
                                    .when_some(save_icon, |button, icon| button.icon(icon))
                                    .label(save_label)
                                    .text_color(save_color)
                                    .tooltip("Save now")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.flush(cx);
                                    })),
                            ),
                    )
                    .child(
                        div().h_flex().child(
                            TabBar::new("editor-view")
                                .segmented()
                                .small()
                                .children(["Source", "Split", "Preview"])
                                .selected_index(mode_index)
                                .last_empty_space(div())
                                .on_click(cx.listener(move |this, index: &usize, window, cx| {
                                    if let Some(mode) = modes.get(*index) {
                                        this.set_mode(*mode, window, cx);
                                    }
                                })),
                        ),
                    ),
            )
            .child(div().flex_1().min_h_0().overflow_hidden().child(body))
            .into_any_element()
    }

    fn status_menu(&self, note: &Note, cx: &Context<Self>) -> impl IntoElement {
        let statuses = self.statuses.clone();
        let status_id = note.status_id.clone();
        let note = note.id.clone();
        let owner = cx.entity().downgrade();
        let label = statuses
            .iter()
            .find(|s| Some(&s.id) == status_id.as_ref())
            .map_or("No status", |s| &s.name);
        Button::new("note-status")
            .small()
            .outline()
            .max_w(rems(10.))
            .label(label.to_owned())
            .dropdown_menu(move |menu, _, _| {
                std::iter::once((None, "No status".to_owned()))
                    .chain(
                        statuses
                            .iter()
                            .map(|s| (Some(s.id.clone()), s.name.clone())),
                    )
                    .fold(menu, |menu, (id, label)| {
                        let note = note.clone();
                        let owner = owner.clone();
                        menu.item(PopupMenuItem::new(label).checked(id == status_id).on_click(
                            move |_, window, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    this.mutate(
                                        |db| db.set_note_status(&note, id.as_deref()),
                                        window,
                                        cx,
                                    );
                                });
                            },
                        ))
                    })
            })
    }

    fn tag_menu(&self, note: &Note, cx: &Context<Self>) -> impl IntoElement {
        let tags = self.tags.clone();
        let selected = note.tag_ids.clone();
        let note = note.id.clone();
        let owner = cx.entity().downgrade();
        let label = if selected.is_empty() {
            "Tags".to_owned()
        } else {
            tags.iter()
                .filter(|t| selected.contains(&t.id))
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        Button::new("note-tags")
            .small()
            .outline()
            .max_w(rems(10.))
            .label(label)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu;
                for tag in &tags {
                    let owner = owner.clone();
                    let id = tag.id.clone();
                    let note = note.clone();
                    menu = menu.item(
                        PopupMenuItem::new(tag.name.clone())
                            .checked(selected.contains(&id))
                            .on_click(move |_, window, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    this.mutate(
                                        |db| {
                                            let mut ids = db.get_note(&note)?.tag_ids;
                                            toggle_id(&mut ids, &id);
                                            db.set_note_tags(&note, &ids)
                                        },
                                        window,
                                        cx,
                                    );
                                });
                            }),
                    );
                }
                let owner = owner.clone();
                let note = note.clone();
                menu.separator()
                    .item(
                        PopupMenuItem::new("New tag…").on_click(move |_, window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.metadata_dialog(
                                    super::dialogs::Metadata::Tag,
                                    None,
                                    Some(note.clone()),
                                    window,
                                    cx,
                                )
                            });
                        }),
                    )
            })
    }
}

fn document_pane_heading(label: &'static str, cx: &App) -> impl IntoElement {
    div()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label)
}

fn notebook_move_menu(
    mut menu: PopupMenu,
    notebook: &Notebook,
    owner: &WeakEntity<Workspace>,
    cx: &App,
) -> PopupMenu {
    let Some(view) = owner.upgrade() else {
        return menu;
    };
    let notebooks = &view.read(cx).notebooks;
    let siblings: Vec<_> = notebooks
        .iter()
        .filter(|item| item.parent_id == notebook.parent_id)
        .collect();
    if let Some(index) = siblings.iter().position(|item| item.id == notebook.id) {
        let previous = index
            .checked_sub(1)
            .and_then(|previous| siblings.get(previous));
        let next = siblings.get(index + 1);
        for (label, neighbor, position) in [
            ("Move up", previous, DropPosition::Before),
            ("Move down", next, DropPosition::After),
        ] {
            let Some(target) = neighbor.map(|item| item.id.clone()) else {
                continue;
            };
            menu = menu.item(move_item(
                label,
                notebook.id.clone(),
                target,
                position,
                owner.clone(),
            ));
        }
    }
    if notebook.parent_id.is_some() {
        let id = notebook.id.clone();
        let owner = owner.clone();
        menu = menu.item(
            PopupMenuItem::new("Move to top level").on_click(move |_, window, cx| {
                let _ = owner.update(cx, |view, cx| {
                    let end = view
                        .notebooks
                        .iter()
                        .filter(|item| item.parent_id.is_none())
                        .count();
                    view.reposition_notebook(&id, None, end, window, cx);
                });
            }),
        );
    }
    let mut excluded = HashSet::from([notebook.id.as_str()]);
    let mut children = std::collections::HashMap::<&str, Vec<&str>>::new();
    for item in notebooks {
        if let Some(parent) = item.parent_id.as_deref() {
            children.entry(parent).or_default().push(&item.id);
        }
    }
    let mut pending = vec![notebook.id.as_str()];
    while let Some(parent) = pending.pop() {
        for id in children.get(parent).into_iter().flatten() {
            if excluded.insert(id) {
                pending.push(id);
            }
        }
    }
    let destinations = notebooks.iter().filter(|item| {
        notebook.parent_id.as_ref() != Some(&item.id) && !excluded.contains(item.id.as_str())
    });
    for target in destinations {
        menu = menu.item(move_item(
            format!("Move into {}", target.name),
            notebook.id.clone(),
            target.id.clone(),
            DropPosition::Inside,
            owner.clone(),
        ));
    }
    menu
}

fn move_item(
    label: impl Into<SharedString>,
    id: String,
    target: String,
    position: DropPosition,
    owner: WeakEntity<Workspace>,
) -> PopupMenuItem {
    PopupMenuItem::new(label).on_click(move |_, window, cx| {
        let _ = owner.update(cx, |view, cx| {
            view.move_notebook(&id, &target, position, window, cx);
        });
    })
}

fn toggle_id(ids: &mut Vec<String>, id: &str) {
    if let Some(index) = ids.iter().position(|value| value == id) {
        ids.remove(index);
    } else {
        ids.push(id.to_owned());
    }
}

pub(super) fn note_title(title: &str) -> &str {
    if title.trim().is_empty() {
        "Untitled"
    } else {
        title
    }
}
