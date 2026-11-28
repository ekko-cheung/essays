use gpui_kit::{
    Bounds, ContentMask, Context, Entity, EntityInputHandler, FontStyle, FontWeight,
    HighlightStyle, Hsla, IntoElement, SharedString, StrikethroughStyle, Styled, UnderlineStyle,
    Window,
    base::input::{InlineToken, InputContent, Rope, RopeExt, TextDecoration},
    canvas,
    component::{
        ActiveTheme,
        highlighter::{HighlightTheme, SyntaxHighlighter},
        input::TextareaState,
    },
    fill, point, px,
};
use markdown::mdast::Node;
use std::{ops::Range, sync::Arc};
use tree_sitter::{InputEdit, Point};

#[derive(Default)]
pub(super) struct LivePreview {
    source: SharedString,
    marks: Vec<(Range<usize>, Mark)>,
    syntax: Vec<(Range<usize>, &'static str)>,
    code_blocks: Vec<CodeBlock>,
    code_highlighter: Option<SyntaxHighlighter>,
    code_theme: Option<Arc<HighlightTheme>>,
    code_styles: Vec<TextDecoration>,
    editing: Range<usize>,
    content: Option<InputContent>,
    decorations: Vec<TextDecoration>,
    colors: Option<[Hsla; 3]>,
}

struct CodeBlock {
    source: Range<usize>,
    body: Range<usize>,
    fences: Vec<Range<usize>>,
    has_language: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mark {
    Bold,
    Italic,
    Strike,
    Link,
    Code,
    Quote,
}

impl LivePreview {
    pub(super) fn update(
        &mut self,
        state: &mut TextareaState,
        window: &mut Window,
        cx: &mut Context<TextareaState>,
    ) -> bool {
        if state.marked_text_range(window, cx).is_some() {
            return false;
        }
        let source_changed = state.text().slice(..) != self.source.as_ref();
        if source_changed {
            let mut preview = Self::parse(state.value());
            preview.code_highlighter = self.code_highlighter.take();
            *self = preview;
        }

        let source = self.source.as_ref();
        let selection = state.selected_range();
        let editing = source[..selection.start].rfind('\n').map_or(0, |at| at + 1)
            ..source[selection.end..]
                .find('\n')
                .map_or(source.len(), |at| selection.end + at);
        let editing_changed = self.content.is_none() || self.editing != editing;
        if editing_changed {
            self.editing = editing;
            self.content = Some(self.content_for_preview());
        }
        let content = self.content.as_ref().unwrap();
        if state.tokens() != content.tokens() && state.set_tokens(content.clone(), cx).is_err() {
            // A secondary cursor can still be inside a marker. Keep it editable.
            let _ = state.set_tokens(InputContent::new(self.source.clone()), cx);
        }

        let theme = cx.theme();
        let code_theme = theme.highlight_theme.clone();
        let code_theme_changed = self
            .code_theme
            .as_ref()
            .is_none_or(|previous| !Arc::ptr_eq(previous, &code_theme));
        if code_theme_changed {
            self.code_styles = self.highlight_code(state.text(), &code_theme);
            self.code_theme = Some(code_theme);
        }
        let colors = [
            theme.link,
            super::theme::markdown_code_background(cx),
            theme.muted_foreground,
        ];
        if editing_changed || code_theme_changed || self.colors != Some(colors) {
            self.colors = Some(colors);
            self.decorations = self.decorations_for_preview(colors);
        }
        if state.decorations() != self.decorations {
            state.set_decorations(self.decorations.clone(), cx);
        }
        source_changed
    }

    pub(super) fn code_backgrounds(
        &self,
        editor: &Entity<TextareaState>,
    ) -> Option<impl IntoElement + use<>> {
        if self.code_blocks.is_empty() {
            return None;
        }
        let ranges: Vec<_> = self
            .code_blocks
            .iter()
            .map(|block| block.source.clone())
            .collect();
        let editor = editor.clone();
        // Paint behind the textarea, after its prepaint has published the current layout.
        Some(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, cx| {
                    let state = editor.read(cx);
                    let (Some(text_bounds), Some(line_height), Some(visible)) = (
                        state.text_bounds(),
                        state.line_height(),
                        state.visible_row_range(),
                    ) else {
                        return;
                    };
                    let viewport = bounds.intersect(&text_bounds);
                    let color = super::theme::markdown_code_background(cx);
                    let text = state.text();
                    let map = state.display_map();
                    let visible_start = text.line_start_offset(visible.start);
                    let visible_end = text.line_start_offset(visible.end);
                    let first_block = ranges.partition_point(|range| range.end <= visible_start);
                    let row_y = |row: usize| {
                        text_bounds.top()
                            + px((row as f64 * f64::from(f32::from(line_height))
                                + state.scroll_y_f64()) as f32)
                    };
                    window.with_content_mask(Some(ContentMask { bounds: viewport }), |window| {
                        for range in ranges[first_block..]
                            .iter()
                            .take_while(|range| range.start < visible_end)
                        {
                            let first = text.offset_to_point(range.start).row;
                            let last = text.offset_to_point(range.end.saturating_sub(1)).row;
                            if let (Some(first), Some(last)) = (
                                map.buffer_line_to_display_row_range(first),
                                map.buffer_line_to_display_row_range(last),
                            ) {
                                let rect = Bounds::from_corners(
                                    point(viewport.left(), row_y(first.start)),
                                    point(viewport.right(), row_y(last.end)),
                                );
                                window.paint_quad(fill(rect, color).corner_radii(px(4.)));
                            }
                        }
                    });
                },
            )
            .absolute()
            .inset_0(),
        )
    }

    fn decorations_for_preview(&self, [link, code, quote]: [Hsla; 3]) -> Vec<TextDecoration> {
        self.marks
            .iter()
            .flat_map(|(range, mark)| {
                let style = match mark {
                    Mark::Bold => HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..Default::default()
                    },
                    Mark::Italic => HighlightStyle {
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    },
                    Mark::Strike => HighlightStyle {
                        strikethrough: Some(StrikethroughStyle {
                            thickness: px(1.),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    Mark::Link => HighlightStyle {
                        color: Some(link),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    Mark::Code => HighlightStyle {
                        background_color: Some(code),
                        ..Default::default()
                    },
                    Mark::Quote => HighlightStyle {
                        color: Some(quote),
                        ..Default::default()
                    },
                };
                [
                    range.start..range.end.min(self.editing.start),
                    range.start.max(self.editing.end)..range.end,
                ]
                .into_iter()
                .filter(|range| !range.is_empty())
                .map(move |range| TextDecoration::new(range, style))
            })
            .chain(self.code_styles.iter().cloned())
            .collect()
    }

    fn highlight_code(&mut self, text: &Rope, theme: &HighlightTheme) -> Vec<TextDecoration> {
        if !self
            .code_blocks
            .iter()
            .any(|block| block.has_language && !block.body.is_empty())
        {
            return Vec::new();
        }
        let highlighter = self
            .code_highlighter
            .get_or_insert_with(|| SyntaxHighlighter::new("markdown"));
        let old_text = highlighter.text();
        let end_point = |text: &Rope| {
            let end = text.offset_to_point(text.len());
            Point::new(end.row, end.column)
        };
        let edit = InputEdit {
            start_byte: 0,
            old_end_byte: old_text.len(),
            new_end_byte: text.len(),
            start_position: Point::new(0, 0),
            old_end_position: end_point(old_text),
            new_end_position: end_point(text),
        };
        highlighter.update(Some(edit), text, None);

        // The Markdown grammar injects each fence's language at source offsets.
        highlighter
            .styles(&(0..text.len()), theme)
            .into_iter()
            .filter(|(_, style)| *style != HighlightStyle::default())
            .flat_map(|(range, style)| {
                let first = self
                    .code_blocks
                    .partition_point(|block| block.body.end <= range.start);
                self.code_blocks[first..]
                    .iter()
                    .take_while(move |block| block.body.start < range.end)
                    .filter(|block| block.has_language && !block.body.is_empty())
                    .map(move |block| {
                        TextDecoration::new(
                            range.start.max(block.body.start)..range.end.min(block.body.end),
                            style,
                        )
                    })
            })
            .collect()
    }

    fn parse(source: SharedString) -> Self {
        let mut preview = Self::default();
        if let Ok(document) = markdown::to_mdast(&source, &markdown::ParseOptions::gfm()) {
            preview.visit(&document, &source);
            preview.syntax.sort_by_key(|(range, _)| range.start);
        }
        preview.source = source;
        preview
    }

    fn content_for_preview(&self) -> InputContent {
        let plain = InputContent::new(self.source.clone());
        self.syntax
            .iter()
            .filter(|(range, _)| range.end <= self.editing.start || range.start >= self.editing.end)
            .map(|(range, label)| (range, *label))
            .chain(
                self.code_blocks
                    .iter()
                    .filter(|block| {
                        block.source.end < self.editing.start
                            || block.source.start > self.editing.end
                    })
                    .flat_map(|block| block.fences.iter().map(|range| (range, "hide"))),
            )
            .try_fold(plain.clone(), |content, (range, label)| {
                let token = InlineToken::new(label, self.source[range.clone()].to_owned());
                content.with_token(range.clone(), token)
            })
            .unwrap_or(plain)
    }

    fn visit(&mut self, node: &Node, source: &str) {
        if let Some(outer) = node_range(node) {
            match node {
                Node::Heading(_)
                | Node::Strong(_)
                | Node::Emphasis(_)
                | Node::Delete(_)
                | Node::Link(_)
                | Node::LinkReference(_) => {
                    let mark = match node {
                        Node::Heading(_) | Node::Strong(_) => Mark::Bold,
                        Node::Emphasis(_) => Mark::Italic,
                        Node::Delete(_) => Mark::Strike,
                        _ => Mark::Link,
                    };
                    if let Some(children) = node.children()
                        && let Some(first) = children.first().and_then(node_range)
                        && let Some(last) = children.last().and_then(node_range)
                    {
                        self.marks.push((first.start..last.end, mark));
                        self.replace(outer.start..first.start, "hide", source);
                        self.replace(last.end..outer.end, "hide", source);
                    }
                }
                Node::InlineCode(_) => {
                    let raw = &source[outer.clone()];
                    let ticks = raw.bytes().take_while(|byte| *byte == b'`').count();
                    let inner = &raw[ticks..raw.len() - ticks];
                    let padding = usize::from(
                        inner.starts_with(' ')
                            && inner.ends_with(' ')
                            && !inner.trim_matches(' ').is_empty(),
                    );
                    let start = outer.start + ticks + padding;
                    let end = outer.end - ticks - padding;
                    self.marks.push((start..end, Mark::Code));
                    self.replace(outer.start..start, "hide", source);
                    self.replace(end..outer.end, "hide", source);
                }
                Node::Code(code) => self.code_blocks.push(CodeBlock::new(outer, code, source)),
                Node::Blockquote(_) => {
                    if source[outer.clone()].starts_with('>') {
                        self.replace(outer.start..outer.start + 1, "│", source);
                    }
                    self.marks.push((outer, Mark::Quote));
                }
                Node::ListItem(item) => {
                    let start = outer.start;
                    let raw = &source[outer];
                    if let Some(checked) = item.checked
                        && let Some(at) = raw.find(']')
                        && let Some(marker) = raw.get(..=at)
                        && marker.contains('[')
                    {
                        self.replace(
                            start..start + marker.len(),
                            if checked { "☑" } else { "☐" },
                            source,
                        );
                    } else if raw.starts_with(['-', '+', '*']) {
                        self.replace(start..start + 1, "•", source);
                    }
                }
                Node::ThematicBreak(_) => self.replace(outer, "────", source),
                _ => {}
            }
        }
        if let Some(children) = node.children() {
            for child in children {
                self.visit(child, source);
            }
        }
    }

    fn replace(&mut self, range: Range<usize>, label: &'static str, source: &str) {
        if let Some(marker) = source.get(range.clone())
            && !marker.is_empty()
            && !marker
                .chars()
                .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        {
            self.syntax.push((range, label));
        }
    }
}

impl CodeBlock {
    fn new(range: Range<usize>, code: &markdown::mdast::Code, source: &str) -> Self {
        let raw = &source[range.clone()];
        let mut block = Self {
            source: range.clone(),
            body: range.clone(),
            fences: Vec::new(),
            has_language: code.lang.is_some(),
        };
        let Some(marker) = raw
            .bytes()
            .next()
            .filter(|byte| matches!(byte, b'`' | b'~'))
        else {
            return block;
        };
        let fence_len = raw.bytes().take_while(|byte| *byte == marker).count();
        if fence_len < 3 {
            return block;
        }

        let opening_end = raw.find('\n').unwrap_or(raw.len());
        let opener = raw[..opening_end].trim_end_matches(['\r', ' ', '\t']);
        block.add_fence(range.start..range.start + opener.len(), source);
        block.body.start = (range.start + opening_end + 1).min(range.end);
        let after_opening = &source[block.body.start..range.end];
        let body_lines = if code.value.is_empty() {
            0
        } else {
            code.value.split('\n').count()
        };
        if !after_opening.ends_with('\n') && after_opening.split('\n').count() > body_lines {
            let last_start = after_opening
                .rfind('\n')
                .map_or(block.body.start, |at| block.body.start + at + 1);
            let last = source[last_start..range.end].trim_end_matches(['\r', ' ', '\t']);
            let closing_len = last
                .bytes()
                .rev()
                .take_while(|byte| *byte == marker)
                .count();
            if closing_len >= fence_len {
                let closing_end = last_start + last.len();
                block.add_fence(closing_end - closing_len..closing_end, source);
                block.body.end = last_start;
            }
        }
        block
    }

    fn add_fence(&mut self, range: Range<usize>, source: &str) {
        // Inline tokens cannot span control characters, including tabs in info strings.
        let separator = |c: char| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}');
        let mut start = range.start;
        for chunk in source[range].split_inclusive(separator) {
            let part = chunk.trim_end_matches(separator);
            if !part.is_empty() {
                self.fences.push(start..start + part.len());
            }
            start += chunk.len();
        }
    }
}

fn node_range(node: &Node) -> Option<Range<usize>> {
    let position = node.position()?;
    Some(position.start.offset..position.end.offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_preview_keeps_source_and_reveals_the_editing_line() {
        let source =
            "# 标题\n\n***你好*** [链接](https://example.com) ~~旧~~ `code`\n\n- [x] 完成\n";
        let mut preview = LivePreview::parse(source.into());
        let start = source.find("***").unwrap();
        preview.editing = start..source[start..].find('\n').unwrap() + start;

        let content = preview.content_for_preview();
        assert_eq!(content.text().as_ref(), source);
        assert_eq!(
            content
                .tokens()
                .iter()
                .map(|span| span.token().text().as_ref())
                .collect::<Vec<_>>(),
            ["# ", "- [x]"]
        );

        preview.editing = 0..0;
        let content = preview.content_for_preview();
        assert_eq!(
            content
                .tokens()
                .iter()
                .map(|span| span.token().text().as_ref())
                .collect::<Vec<_>>(),
            [
                "# ",
                "*",
                "**",
                "**",
                "*",
                "[",
                "](https://example.com)",
                "~~",
                "~~",
                "`",
                "`",
                "- [x]"
            ]
        );
    }

    #[test]
    fn escaped_markup_and_fenced_code_keep_their_source() {
        let source = "\\*literal\\*\n\n```markdown\n**literal**\n```";
        let mut preview = LivePreview::parse(source.into());
        let cursor = source.find("literal**").unwrap();
        preview.editing = cursor..cursor;
        assert!(preview.content_for_preview().tokens().is_empty());
        assert!(preview.marks.is_empty());
        assert_eq!(
            &source[preview.code_blocks[0].body.clone()],
            "**literal**\n"
        );
    }

    #[test]
    fn code_preview_hides_only_inactive_fences_and_keeps_source() {
        let source =
            "Notes\r\n\r\n> ~~~~js\r\n> const a = 1;\r\n> ~~~~  \r\n\r\n```rust\nlast line";
        let mut preview = LivePreview::parse(source.into());
        let content = preview.content_for_preview();
        assert_eq!(content.text().as_ref(), source);
        let hidden: Vec<_> = content
            .tokens()
            .iter()
            .filter(|span| span.token().id().as_ref() == "hide")
            .map(|span| span.token().text().as_ref())
            .collect();
        assert_eq!(hidden, ["~~~~js", "~~~~", "```rust"]);
        let cursor = source.find("const").unwrap();
        preview.editing = cursor..cursor;
        let content = preview.content_for_preview();
        let hidden: Vec<_> = content
            .tokens()
            .iter()
            .filter(|span| span.token().id().as_ref() == "hide")
            .map(|span| span.token().text().as_ref())
            .collect();
        assert_eq!(hidden, ["```rust"]);
        assert_eq!(&source[preview.code_blocks[1].body.clone()], "last line");
    }

    #[test]
    fn live_code_uses_language_colors_at_source_offsets_in_both_themes() {
        let source = "# Heading\n\n```rs\nfn main() { let a = \"你好\"; }\n```\n\n> ```python\n> def greet():\n>     return 42\n> ```";
        let mut preview = LivePreview::parse(source.into());
        for theme in [
            HighlightTheme::default_light(),
            HighlightTheme::default_dark(),
        ] {
            let styles = preview.highlight_code(&Rope::from(source), &theme);
            for keyword in ["fn", "def", "return"] {
                let start = source.find(keyword).unwrap();
                assert!(
                    styles.iter().any(|decoration| {
                        decoration.range.start <= start
                            && decoration.range.end >= start + keyword.len()
                            && decoration.style.color
                                == theme.style("keyword").and_then(|style| style.color)
                            && decoration.style.color.is_some()
                    }),
                    "{keyword} should have a keyword color: {styles:?}"
                );
            }
            assert!(
                styles
                    .iter()
                    .all(|decoration| decoration.range.start >= source.find("fn").unwrap())
            );
        }
    }
}
