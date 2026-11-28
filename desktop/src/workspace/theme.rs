use crate::settings::Appearance;
use gpui_kit::{
    App, HighlightStyle, Hsla, StyleRefinement, Styled, Window,
    component::{ActiveTheme, Theme, ThemeMode, text::TextViewStyle},
    rgb,
};

struct Palette {
    editor: u32,
    list: u32,
    sidebar: u32,
    foreground: u32,
    secondary: u32,
    muted: u32,
    border: u32,
    accent: u32,
    action: u32,
    on_accent: u32,
    danger: u32,
    code_background: u32,
    code_foreground: u32,
    selection_tint: u32,
    selected_opacity: f32,
}

const LIGHT: Palette = Palette {
    editor: 0xffffff,
    list: 0xfafbfc,
    sidebar: 0xf2f3f7,
    foreground: 0x1d2129,
    secondary: 0x4e5663,
    muted: 0x626b7b,
    border: 0xe6e8ed,
    accent: 0x4f6df5,
    action: 0x405ed8,
    on_accent: 0xffffff,
    danger: 0xc8323c,
    code_background: 0xf0f1f5,
    code_foreground: 0xb4308b,
    selection_tint: 0x4f6df5,
    selected_opacity: 0.12,
};

const DARK: Palette = Palette {
    editor: 0x1e1f24,
    list: 0x1a1b20,
    sidebar: 0x141519,
    foreground: 0xe7e9ee,
    secondary: 0xb4b9c4,
    muted: 0x9ba2b0,
    border: 0x2a2c33,
    accent: 0x7b94ff,
    action: 0x7b94ff,
    on_accent: 0x10131c,
    danger: 0xff6369,
    code_background: 0x272930,
    code_foreground: 0xf08fd0,
    selection_tint: 0x6f8cff,
    selected_opacity: 0.18,
};

pub(super) fn apply_appearance(appearance: Appearance, window: &mut Window, cx: &mut App) {
    let mode = match appearance {
        Appearance::System => ThemeMode::from(window.appearance()),
        Appearance::Light => ThemeMode::Light,
        Appearance::Dark => ThemeMode::Dark,
    };
    Theme::change(mode, Some(window), cx);
    let palette = if mode.is_dark() { &DARK } else { &LIGHT };
    Theme::update(cx, |theme| {
        let colors = &mut theme.colors;
        let accent = gpui_kit::Hsla::from(rgb(palette.accent));
        colors.background = rgb(palette.editor).into();
        colors.foreground = rgb(palette.foreground).into();
        colors.muted_foreground = rgb(palette.muted).into();
        colors.border = rgb(palette.border).into();
        colors.input = colors.border;
        colors.accent = if mode.is_dark() {
            gpui_kit::white().opacity(0.06)
        } else {
            gpui_kit::Hsla::from(rgb(0x0f172a)).opacity(0.05)
        };
        colors.accent_foreground = colors.foreground;
        colors.muted = colors.background.blend(colors.accent);
        colors.secondary = colors.muted;
        colors.secondary_foreground = rgb(palette.secondary).into();
        colors.primary = rgb(palette.action).into();
        colors.primary_foreground = rgb(palette.on_accent).into();
        colors.primary_hover = colors
            .primary
            .blend(colors.primary_foreground.opacity(0.07));
        colors.primary_active = colors
            .primary
            .blend(colors.primary_foreground.opacity(0.15));
        colors.button_primary = colors.primary;
        colors.button_primary_hover = colors.primary_hover;
        colors.button_primary_active = colors.primary_active;
        colors.button_primary_foreground = colors.primary_foreground;
        colors.ring = accent;
        colors.caret = accent;
        colors.link = colors.primary;
        colors.link_hover = colors.primary_hover;
        colors.link_active = colors.primary_active;
        colors.selection = accent.opacity(0.30);
        colors.danger = rgb(palette.danger).into();
        colors.button_danger = colors.danger;
        colors.button_danger_foreground = if mode.is_dark() {
            rgb(0x1a0b0c).into()
        } else {
            gpui_kit::white()
        };
        colors.button_danger_hover = colors
            .button_danger
            .blend(colors.button_danger_foreground.opacity(0.07));
        colors.button_danger_active = colors
            .button_danger
            .blend(colors.button_danger_foreground.opacity(0.15));
        colors.overlay = Hsla::from(rgb(0x0a0c14)).opacity(0.45);
        colors.scrollbar_thumb = colors.muted_foreground.opacity(0.40);
        colors.scrollbar_thumb_hover = colors.muted_foreground.opacity(0.65);
        colors.sidebar = rgb(palette.sidebar).into();
        colors.sidebar_foreground = colors.foreground;
        colors.sidebar_border = colors.border;
        colors.sidebar_accent =
            Hsla::from(rgb(palette.selection_tint)).opacity(palette.selected_opacity);
        colors.sidebar_accent_foreground = colors.foreground;
        colors.list = rgb(palette.list).into();
        colors.list_head = colors.list;
        colors.list_hover = colors.accent;
        colors.list_active = colors.sidebar_accent;
        colors.list_active_border = accent.opacity(0.30);
        colors.tab_bar_segmented = colors.accent;
        colors.tab_active = colors.background;
        colors.tab_active_foreground = colors.foreground;
        colors.tab_foreground = colors.muted_foreground;
        colors.popover = colors.background;
        colors.popover_foreground = colors.foreground;
        colors.table_head = colors.muted;
        colors.table_head_foreground = colors.foreground;
    });
}

pub(super) fn metadata_color(color: &str) -> Option<Hsla> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16)
        .ok()
        .map(|value| rgb(value).into())
}

pub(super) fn markdown_preview_style(cx: &App) -> TextViewStyle {
    let theme = cx.theme();
    let palette = if theme.is_dark() { &DARK } else { &LIGHT };
    let code_bg = Hsla::from(rgb(palette.code_background));
    TextViewStyle::default()
        .code_block(
            StyleRefinement::default()
                .bg(code_bg)
                .rounded(theme.radius_lg),
        )
        .inline_code(HighlightStyle {
            color: Some(rgb(palette.code_foreground).into()),
            background_color: Some(code_bg),
            ..Default::default()
        })
}
