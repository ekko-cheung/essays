use super::*;
use chrono::{DateTime, Local, Utc};
use gpui_kit::assets::IconName as WorkspaceIcon;
use gpui_kit::component::{
    Icon, IconName, Selectable, Sizable,
    checkbox::Checkbox,
    input::{Editor, Input},
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    tab::{Tab, TabBar},
    text::TextView,
    tooltip::Tooltip,
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
            .pl_4()
            .pr_3()
            .pt_3()
            .pb_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted_foreground)
                    .child("NOTEBOOKS"),
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
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .child(toolbar)
            .child(
                div()
                    .id("notebook-tree")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_0p5()
                    .px_2()
                    .overflow_y_scroll()
                    .child(
                        Button::new("all-notes")
                            .ghost()
                            .small()
                            .w_full()
                            .accessibility_label("All notes")
                            .selected(self.query.notebook_id.is_none())
                            .when(self.query.notebook_id.is_none(), |button| {
                                button
                                    .bg(cx.theme().sidebar_accent)
                                    .text_color(cx.theme().sidebar_accent_foreground)
                            })
                            .child(
                                div()
                                    .h_flex()
                                    .flex_1()
                                    .gap_1()
                                    .pl_6()
                                    .child(workspace_icon(WorkspaceIcon::Layers).text_color(
                                        if self.query.notebook_id.is_none() {
                                            cx.theme().ring
                                        } else {
                                            cx.theme().muted_foreground
                                        },
                                    ))
                                    .child(div().truncate().child("All notes")),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select_notebook(None, window, cx)
                            })),
                    )
                    .children(self.notebook_rows(cx))
                    .child(self.filters(cx)),
            )
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .gap_1()
                    .p_2()
                    .border_t_1()
                    .border_color(cx.theme().sidebar_border)
                    .child(
                        Button::new("settings")
                            .ghost()
                            .small()
                            .flex_1()
                            .text_color(cx.theme().secondary_foreground)
                            .accessibility_label("Settings")
                            .child(
                                div()
                                    .h_flex()
                                    .flex_1()
                                    .gap_2()
                                    .child(Icon::new(IconName::Settings).small())
                                    .child("Settings…"),
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.settings_dialog(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("help")
                            .ghost()
                            .small()
                            .icon(workspace_icon(WorkspaceIcon::CircleQuestionMark))
                            .accessibility_label("Keyboard shortcuts")
                            .tooltip("Keyboard shortcuts…")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.help_dialog(window, cx)),
                            ),
                    ),
            )
    }

    fn filters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use super::dialogs::Metadata;

        let active = !self.query.status_ids.is_empty()
            || !self.query.tag_ids.is_empty()
            || self.query.no_status;
        let status_rows = self.statuses.iter().map(|status| {
            let id = status.id.clone();
            filter_row(
                format!("filter-status-{id}"),
                &status.name,
                status_dot(Some(status), cx),
                Some(status.note_count),
                self.query.status_ids.contains(&id),
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                toggle_id(&mut this.query.status_ids, &id);
                this.refresh_notes(cx);
            }))
        });
        let tag_rows = self.tags.iter().map(|tag| {
            let id = tag.id.clone();
            filter_row(
                format!("filter-tag-{id}"),
                &tag.name,
                workspace_icon(WorkspaceIcon::Tag).text_color(
                    super::theme::metadata_color(&tag.color).unwrap_or(cx.theme().muted_foreground),
                ),
                Some(tag.note_count),
                self.query.tag_ids.contains(&id),
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                toggle_id(&mut this.query.tag_ids, &id);
                this.refresh_notes(cx);
            }))
        });
        div()
            .v_flex()
            .child(
                filter_section(cx)
                    .child(
                        div()
                            .h_flex()
                            .px_2()
                            .child(section_heading("STATUS", cx))
                            .when(active, |header| {
                                header.child(
                                    Button::new("clear-filters")
                                        .link()
                                        .xsmall()
                                        .label("Clear")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.query.status_ids.clear();
                                            this.query.tag_ids.clear();
                                            this.query.no_status = false;
                                            this.refresh_notes(cx);
                                        })),
                                )
                            })
                            .child(
                                Button::new("manage-statuses")
                                    .ghost()
                                    .xsmall()
                                    .icon(workspace_icon(WorkspaceIcon::SlidersVertical))
                                    .accessibility_label("Manage statuses")
                                    .tooltip("Manage statuses")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.manage_dialog(Metadata::Status, window, cx);
                                    })),
                            ),
                    )
                    .children(status_rows)
                    .child(
                        filter_row(
                            "filter-no-status".into(),
                            "No status",
                            status_dot(None, cx),
                            None,
                            self.query.no_status,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.query.no_status = !this.query.no_status;
                            this.refresh_notes(cx);
                        })),
                    ),
            )
            .child(
                filter_section(cx)
                    .child(
                        div()
                            .h_flex()
                            .px_2()
                            .child(section_heading("TAGS", cx))
                            .child(
                                Button::new("manage-tags")
                                    .ghost()
                                    .xsmall()
                                    .icon(workspace_icon(WorkspaceIcon::SlidersVertical))
                                    .accessibility_label("Manage tags")
                                    .tooltip("Manage tags")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.manage_dialog(Metadata::Tag, window, cx);
                                    })),
                            ),
                    )
                    .children(tag_rows)
                    .when(self.tags.is_empty(), |section| {
                        section.child(
                            div()
                                .px_2()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("No tags yet."),
                        )
                    }),
            )
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
            .rounded(cx.theme().radius)
            .when(selected, |row| row.bg(cx.theme().sidebar_accent))
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
                    .when(selected, |button| button.bg(transparent_black()))
                    .min_w_0()
                    .flex_1()
                    .accessibility_label(notebook.name.clone())
                    .icon(workspace_icon(WorkspaceIcon::Book).text_color(if selected {
                        cx.theme().ring
                    } else {
                        cx.theme().muted_foreground
                    }))
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
        let ring = cx.theme().ring;
        // UniformList measures a single row. Reserve a chip line whenever the
        // workspace has tagged notes so compact panes can keep the labels readable.
        let row_height = rems(if self.tags.iter().any(|tag| tag.note_count > 0) {
            6.
        } else {
            4.75
        });
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
                    .gap_1()
                    .pl_4()
                    .pr_3()
                    .py_3()
                    .bg(cx.theme().list_head)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(rems(0.9375))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(heading.to_owned()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_2()
                            .rounded_full()
                            .bg(cx.theme().accent)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.notes.len().to_string()),
                    )
                    .child(
                        Button::new("new-note")
                            .ghost()
                            .small()
                            .icon(workspace_icon(WorkspaceIcon::FilePlus))
                            .accessibility_label("New note")
                            .tooltip("New note")
                            .on_click(cx.listener(|this, _, window, cx| this.new_note(window, cx))),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .px_3()
                    .pb_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Input::new(&self.search)
                            .aria_label("Search notes")
                            .bg(cx.theme().background)
                            .cleanable(true)
                            .prefix(
                                Icon::new(IconName::Search)
                                    .small()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .when(self.query.notebook_id.is_some(), |bar| {
                        bar.child(
                            Checkbox::new("include-sub-notebooks")
                                .small()
                                .text_xs()
                                .label("Include sub-notebooks")
                                .checked(self.query.include_descendants)
                                .on_click(cx.listener(|this, &include, _, cx| {
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
                                })),
                        )
                    }),
            )
            .when(self.notes.is_empty(), |pane| {
                let filtered = !self.query.search.is_empty()
                    || !self.query.tag_ids.is_empty()
                    || !self.query.status_ids.is_empty()
                    || self.query.no_status;
                pane.child(
                    div()
                        .v_flex()
                        .items_center()
                        .gap_3()
                        .px_4()
                        .py_10()
                        .text_sm()
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            Icon::new(if filtered {
                                IconName::Search
                            } else {
                                IconName::FileText
                            })
                            .size_7(),
                        )
                        .child(if filtered {
                            "No notes match the current search or filters."
                        } else {
                            "No notes yet."
                        }),
                )
            })
            .when(!self.notes.is_empty(), |pane| {
                pane.child(
                    uniform_list(
                        "notes",
                        self.notes.len(),
                        cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                            this.notes[range]
                                .iter()
                                .map(|note| this.note_row(note, row_height, cx))
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

    fn note_row(&self, note: &NoteSummary, row_height: Rems, cx: &Context<Self>) -> AnyElement {
        let id: SharedString = note.id.clone().into();
        let selected = self
            .note
            .as_ref()
            .is_some_and(|current| current.id == note.id);
        let title = note_title(&note.title);
        let snippet = note
            .snippet
            .chars()
            .filter(|ch| !matches!(ch, '#' | '>' | '*' | '_' | '`' | '~' | '[' | ']'))
            .collect::<String>();
        let snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
        let notebook = (self.query.notebook_id.as_ref() != Some(&note.notebook_id))
            .then(|| self.notebooks.iter().find(|n| n.id == note.notebook_id))
            .flatten();
        let status = self
            .statuses
            .iter()
            .find(|s| Some(&s.id) == note.status_id.as_ref());
        let updated = DateTime::<Utc>::from_timestamp_millis(note.updated_at)
            .map(|date| date.with_timezone(&Local))
            .map(|date| {
                if date.date_naive() == Local::now().date_naive() {
                    date.format("%H:%M").to_string()
                } else {
                    date.format("%x").to_string()
                }
            });
        let open = note.id.clone();
        let pin = note.id.clone();
        let pinned = note.pinned;
        let delete = note.clone();
        let hover = if selected {
            cx.theme().list_active
        } else {
            cx.theme().list_hover
        };
        let focus = self.notes_focus.clone();
        div()
            .id(format!("note-row-{id}"))
            .h(row_height)
            .px_2()
            .py_0p5()
            .child(
                div()
                    .id("note-row-content")
                    .v_flex()
                    .size_full()
                    .pl_3()
                    .pr_2()
                    .py_2()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(transparent_black())
                    .when(selected, |row| {
                        row.bg(cx.theme().list_active)
                            .border_color(cx.theme().list_active_border)
                    })
                    .hover(move |row| row.bg(hover))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        window.focus(&focus, cx);
                        this.select_note(&open, window, cx);
                    }))
                    .child(
                        div()
                            .h_flex()
                            .gap_1()
                            .child(status_dot(status, cx))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .line_height(rems(1.125))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title.to_owned()),
                            )
                            .child(
                                Button::new(format!("pin-note-{id}"))
                                    .ghost()
                                    .xsmall()
                                    .selected(pinned)
                                    .bg(transparent_black())
                                    .icon(workspace_icon(WorkspaceIcon::Pin).text_color(
                                        if pinned {
                                            cx.theme().ring
                                        } else {
                                            cx.theme().muted_foreground
                                        },
                                    ))
                                    .accessibility_label(if pinned {
                                        "Unpin note"
                                    } else {
                                        "Pin note"
                                    })
                                    .tooltip(if pinned { "Unpin note" } else { "Pin note" })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.mutate(
                                            |db| db.set_note_pinned(&pin, !pinned),
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new(format!("delete-note-{id}"))
                                    .ghost()
                                    .xsmall()
                                    .icon(
                                        workspace_icon(WorkspaceIcon::Trash)
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .accessibility_label("Delete note")
                                    .tooltip("Delete note")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.delete_note(delete.clone(), window, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .truncate()
                            .text_size(rems(0.8125))
                            .line_height(rems(1.0625))
                            .text_color(cx.theme().secondary_foreground)
                            .child(if snippet.is_empty() {
                                "No content".into()
                            } else {
                                snippet
                            }),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_1()
                            .overflow_hidden()
                            .text_xs()
                            .line_height(rems(0.9375))
                            .text_color(cx.theme().muted_foreground)
                            .when_some(updated, |row, updated| {
                                row.child(div().flex_shrink_0().child(updated))
                            })
                            .when_some(notebook, |row, notebook| {
                                row.child(
                                    div()
                                        .h_flex()
                                        .min_w_0()
                                        .gap_1()
                                        .px_1()
                                        .rounded_sm()
                                        .bg(cx.theme().accent)
                                        .child(workspace_icon(WorkspaceIcon::Book).xsmall())
                                        .child(div().truncate().child(notebook.name.clone())),
                                )
                            }),
                    )
                    .when(!note.tag_ids.is_empty(), |row| {
                        row.child(
                            div().h_flex().gap_1().mt_1().overflow_hidden().children(
                                self.tags
                                    .iter()
                                    .filter(|tag| note.tag_ids.contains(&tag.id))
                                    .map(|tag| tag_chip(tag, cx).flex_shrink_0()),
                            ),
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
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(IconName::FileText).size_10())
                .child(div().text_sm().text_center().child(if missing_notebooks {
                    "Create your first notebook"
                } else if can_create_note {
                    "Start a note"
                } else {
                    "Select a note"
                }))
                .when(missing_notebooks || can_create_note, |pane| {
                    let label = if missing_notebooks {
                        "New notebook…"
                    } else {
                        "New note"
                    };
                    pane.child(
                        Button::new("empty-workspace-action")
                            .primary()
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
        let (save_label, save_color) = if self.save_failed {
            ("Save failed", cx.theme().danger)
        } else if self.is_dirty(cx) {
            ("Editing…", cx.theme().muted_foreground)
        } else {
            ("Saved", cx.theme().muted_foreground)
        };
        let modes = [View::Source, View::Split, View::Preview];
        let mode_index = modes.iter().position(|value| *value == mode).unwrap_or(1);
        let mode_tabs = TabBar::new("editor-view")
            .segmented()
            .small()
            .children(
                [
                    ("Source", WorkspaceIcon::FileText),
                    ("Split", WorkspaceIcon::Columns2),
                    ("Preview", WorkspaceIcon::Eye),
                ]
                .map(|(label, icon)| {
                    Tab::new().aria_label(label).child(
                        div()
                            .id(label)
                            .h_flex()
                            .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
                            .child(workspace_icon(icon)),
                    )
                }),
            )
            .selected_index(mode_index)
            .last_empty_space(div())
            .on_click(cx.listener(move |this, index: &usize, window, cx| {
                if let Some(mode) = modes.get(*index) {
                    this.set_mode(*mode, window, cx);
                }
            }));
        let preview = || {
            let markdown = self.editor.read(cx).value();
            div()
                .h_flex()
                .justify_center()
                .size_full()
                .min_w_0()
                .min_h_0()
                .px_3()
                .pt_3()
                .when(mode == View::Preview, |pane| pane.px_6())
                .child(
                    TextView::markdown(format!("preview-{}", note.id), markdown)
                        .style(super::theme::markdown_preview_style(cx))
                        .blockquote_style(
                            StyleRefinement::default()
                                .bg(cx.theme().ring.opacity(0.06))
                                .border_l_2()
                                .border_color(cx.theme().ring.opacity(0.55))
                                .text_color(cx.theme().secondary_foreground),
                        )
                        .scrollable(true)
                        .size_full()
                        .max_w(rems(50.))
                        .text_size(rems(0.9375))
                        .line_height(rems(1.625))
                        .on_link_click(|url, _, _, cx| {
                            if url.starts_with("https://")
                                || url.starts_with("http://")
                                || url.starts_with("mailto:")
                            {
                                cx.open_url(url);
                            }
                        }),
                )
        };
        let source = || {
            div()
                .v_flex()
                .size_full()
                .min_w_0()
                .min_h_0()
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if this.service.settings().auto_close_brackets {
                        super::markdown_edit::complete_fence(&this.editor, event, window, cx);
                    }
                }))
                .child(
                    div().flex_1().min_h_0().px_3().pt_2().child(
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
                    .pt_3()
                    .pb_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .h_flex()
                            .min_h_10()
                            .gap_2()
                            .child(div().flex_1().min_w_0().child(if mode == View::Preview {
                                div()
                                    .truncate()
                                    .px_1()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(note_title(&self.title.read(cx).value()).to_owned())
                                    .into_any_element()
                            } else {
                                Input::new(&self.title)
                                    .large()
                                    .appearance(false)
                                    .px_1()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .aria_label("Note title")
                                    .into_any_element()
                            }))
                            .child(
                                Button::new("save-note")
                                    .ghost()
                                    .small()
                                    .flex_shrink_0()
                                    .label(save_label)
                                    .text_color(save_color)
                                    .tooltip("Save now")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.flush(cx);
                                    })),
                            )
                            .child(div().flex_shrink_0().child(mode_tabs)),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(self.status_menu(note, cx))
                            .children(
                                self.tags
                                    .iter()
                                    .filter(|tag| note.tag_ids.contains(&tag.id))
                                    .map(|tag| tag_chip(tag, cx)),
                            )
                            .child(self.tag_menu(note, cx)),
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
        let status = statuses.iter().find(|s| Some(&s.id) == status_id.as_ref());
        let label = status.map_or("No status", |s| &s.name);
        Button::new("note-status")
            .small()
            .secondary()
            .max_w(rems(10.))
            .accessibility_label(label.to_owned())
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .min_w_0()
                    .child(status_dot(status, cx))
                    .child(div().truncate().child(label.to_owned()))
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .xsmall()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
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
        Button::new("note-tags")
            .small()
            .ghost()
            .icon(workspace_icon(WorkspaceIcon::Tag))
            .label("Add tag…")
            .text_color(cx.theme().muted_foreground)
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

fn workspace_icon(icon: WorkspaceIcon) -> Icon {
    Icon::new(icon).small()
}

fn section_heading(label: &'static str, cx: &App) -> Div {
    div()
        .flex_1()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label)
}

fn filter_section(cx: &App) -> Div {
    div()
        .v_flex()
        .flex_shrink_0()
        .mt_2()
        .pt_2()
        .border_t_1()
        .border_color(cx.theme().sidebar_border)
}

fn filter_row(
    id: String,
    name: &str,
    marker: impl IntoElement,
    count: Option<i64>,
    selected: bool,
    cx: &App,
) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .w_full()
        .selected(selected)
        .text_color(cx.theme().secondary_foreground)
        .when(selected, |row| {
            row.bg(cx.theme().sidebar_accent)
                .text_color(cx.theme().sidebar_accent_foreground)
        })
        .child(
            div()
                .h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .child(div().h_flex().justify_center().w_4().child(marker))
                .child(div().flex_1().min_w_0().truncate().child(name.to_owned()))
                .when_some(count, |row, count| {
                    row.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(count.to_string()),
                    )
                }),
        )
}

fn status_dot(status: Option<&Status>, cx: &App) -> Div {
    let color = status.and_then(|status| super::theme::metadata_color(&status.color));
    div()
        .size_2()
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .border_color(color.unwrap_or(cx.theme().muted_foreground))
        .when_some(color, |dot, color| dot.bg(color))
}

fn tag_chip(tag: &Tag, cx: &App) -> Div {
    let color = super::theme::metadata_color(&tag.color).unwrap_or(cx.theme().muted_foreground);
    div()
        .min_w_0()
        .max_w(rems(9.))
        .rounded_full()
        .border_1()
        .border_color(color.opacity(0.55))
        .bg(color.opacity(0.16))
        .px_1p5()
        .text_xs()
        .line_height(rems(0.9375))
        .text_color(cx.theme().foreground)
        .child(div().truncate().child(tag.name.clone()))
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
