use gpui_kit::{
    App, Context, Entity, EntityInputHandler, KeyDownEvent, Window,
    base::input::{
        AutoClosingPair, BracketPair, language_config::LanguageConfig, set_language_config,
    },
    component::input::EditorState,
};
use markdown::mdast::Node;

pub(super) fn register_brackets(cx: &mut App) {
    // Use the Markdown syntax provider with prose-friendly pairing rules.
    let pairs = [("(", ")"), ("[", "]"), ("{", "}")];
    let rules = LanguageConfig::default()
        .brackets(pairs.map(|(open, close)| BracketPair::new(open, close)))
        .auto_closing_pairs(pairs.map(|(open, close)| AutoClosingPair::new(open, close)));
    set_language_config("markdown", rules, cx);
}

pub(super) fn complete_fence(
    editor: &Entity<EditorState>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if event.keystroke.key_char.as_deref() != Some("`") {
        return;
    }
    let completed = editor.update(cx, |state, cx: &mut Context<EditorState>| {
        if !state.selected_range().is_empty() || state.marked_text_range(window, cx).is_some() {
            return false;
        }
        let cursor = state.cursor();
        let text = state.text();
        if cursor < 2
            || !text.is_char_boundary(cursor - 2)
            || text.slice(cursor - 2..cursor) != "``"
        {
            return false;
        }
        if !starts_fence(&state.value(), cursor) {
            return false;
        }
        state.insert("`\n```", window, cx);
        state.set_selected_range(cursor + 1..cursor + 1, cx);
        true
    });
    if completed {
        cx.stop_propagation();
        window.prevent_default();
    }
}

fn starts_fence(text: &str, cursor: usize) -> bool {
    let (before, after) = text.split_at(cursor);
    let line = before.rsplit('\n').next().unwrap_or_default();
    let indent = line.len().saturating_sub(2);
    if indent > 3
        || !line
            .strip_suffix("``")
            .is_some_and(|prefix| prefix.bytes().all(|byte| byte == b' '))
        || !after
            .split('\n')
            .next()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return false;
    }
    let line_start = cursor - line.len();
    markdown::to_mdast(text, &markdown::ParseOptions::gfm())
        .is_ok_and(|document| !inside_code(&document, line_start))
}

fn inside_code(node: &Node, offset: usize) -> bool {
    if !node
        .position()
        .is_some_and(|position| position.start.offset <= offset && offset <= position.end.offset)
    {
        return false;
    }
    matches!(node, Node::Code(_))
        || node
            .children()
            .is_some_and(|children| children.iter().any(|child| inside_code(child, offset)))
}

#[cfg(test)]
mod tests {
    use super::starts_fence;

    #[test]
    fn completes_a_new_fence_without_reopening_existing_code() {
        for text in ["``", "Note\n\n   ``"] {
            assert!(starts_fence(text, text.len()));
        }
        for text in ["text ``", "    ``", "```rust\nabc\n``", "~~~\nabc\n``"] {
            assert!(!starts_fence(text, text.len()));
        }
        assert!(!starts_fence("`` text", 2));
    }
}
