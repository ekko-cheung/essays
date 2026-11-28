use crate::settings::Appearance;
use gpui_kit::{
    App, Window,
    component::{Theme, ThemeMode},
    rgb,
};

struct Palette {
    paper: u32,
    chrome: u32,
    sidebar: u32,
    ink: u32,
    secondary_ink: u32,
    rule: u32,
    input_rule: u32,
    accent: u32,
    on_accent: u32,
    hover: u32,
    selected: u32,
}

const LIGHT: Palette = Palette {
    paper: 0xfdfcf8,
    chrome: 0xf8f7f1,
    sidebar: 0xf0f1e9,
    ink: 0x222820,
    secondary_ink: 0x586155,
    rule: 0xdbdfd4,
    input_rule: 0xa4ad9e,
    accent: 0x315941,
    on_accent: 0xffffff,
    hover: 0xebefe5,
    selected: 0xe0eadc,
};

const DARK: Palette = Palette {
    paper: 0x1a1f1c,
    chrome: 0x1f2521,
    sidebar: 0x242b25,
    ink: 0xe9eee7,
    secondary_ink: 0xb3beb1,
    rule: 0x39443a,
    input_rule: 0x69776a,
    accent: 0xa6cba9,
    on_accent: 0x1c3023,
    hover: 0x2b362d,
    selected: 0x314338,
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
        colors.background = rgb(palette.paper).into();
        colors.foreground = rgb(palette.ink).into();
        colors.muted = rgb(palette.chrome).into();
        colors.muted_foreground = rgb(palette.secondary_ink).into();
        colors.border = rgb(palette.rule).into();
        colors.input = rgb(palette.input_rule).into();
        colors.accent = rgb(palette.hover).into();
        colors.accent_foreground = colors.foreground;
        colors.primary = rgb(palette.accent).into();
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
        colors.ring = colors.primary;
        colors.caret = colors.primary;
        colors.link = colors.primary;
        colors.link_hover = colors.primary_hover;
        colors.link_active = colors.primary_active;
        colors.selection = colors.primary.opacity(0.25);
        colors.sidebar = rgb(palette.sidebar).into();
        colors.sidebar_foreground = colors.foreground;
        colors.sidebar_border = colors.border;
        colors.sidebar_accent = rgb(palette.selected).into();
        colors.sidebar_accent_foreground = colors.primary;
        colors.list = colors.background;
        colors.list_head = colors.muted;
        colors.list_hover = colors.accent;
        colors.list_active = colors.sidebar_accent;
        colors.list_active_border = colors.primary;
        colors.tab_bar_segmented = colors.sidebar;
        colors.tab_active = colors.sidebar_accent;
        colors.tab_active_foreground = colors.primary;
        colors.popover = colors.background;
        colors.popover_foreground = colors.foreground;
    });
}
