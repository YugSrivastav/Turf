//! Top row Tab Strip with sleek Catppuccin / Herdr numbered capsule pills,
//! clickable `+` new-tab trigger, and clean right-aligned session badge.
//!
//! Conforms to [MD-044] and [MD-050].

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::theme::{
    Theme, COLOR_GREEN_DOT, COLOR_HEADER_BG, COLOR_TEXT_DIM, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};

/// Represents a tab metadata descriptor.
#[derive(Debug, Clone)]
pub struct TabItem {
    pub id: usize,
    pub title: String,
    pub foreground_cmd: Option<String>,
    pub scroll_badge: Option<String>,
}

impl TabItem {
    pub fn new(id: usize, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            foreground_cmd: None,
            scroll_badge: None,
        }
    }

    pub fn with_foreground_cmd(mut self, cmd: Option<String>) -> Self {
        self.foreground_cmd = cmd;
        self
    }

    pub fn with_scroll_badge(mut self, badge: Option<String>) -> Self {
        self.scroll_badge = badge;
        self
    }

    /// Format clean Herdr-grade pill text without noisy shell process names (`zsh`).
    pub fn pill_text(&self) -> String {
        let trimmed = self.title.trim();
        let is_default = trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("shell")
            || trimmed.eq_ignore_ascii_case("tab")
            || trimmed == self.id.to_string();

        let base = if is_default {
            format!(" {:<4} ", self.id)
        } else {
            format!(" {} ", trimmed)
        };

        if let Some(badge) = &self.scroll_badge {
            if !badge.is_empty() {
                return format!("{base}{badge} ");
            }
        }
        base
    }
}

/// Interactive hit target inside the top tab strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabStripHit {
    ScrollLeft,
    Tab(usize),
    ScrollRight,
    AddButton,
}

fn right_badge_width(session_code: &str, role: &str, status_hint: Option<&str>) -> u16 {
    let status_len = status_hint.map(|s| s.chars().count() + 3).unwrap_or(0);
    // "● " (2) + code + " • " (3) + role + " " (1)
    (status_len + 2 + session_code.chars().count() + 3 + role.chars().count() + 1) as u16
}

/// Compute clickable bounding rectangles for visible tabs, scroll arrows `<` / `>`, and pinned `+` button.
pub fn compute_tab_strip_hits(area: Rect, tabs: &[TabItem]) -> Vec<(Rect, TabStripHit)> {
    compute_tab_strip_hits_scrolled(area, tabs, 0, "", "", None)
}

/// Compute clickable hit targets with horizontal scrolling and pinned `+` button so any number of tabs works.
pub fn compute_tab_strip_hits_scrolled(
    area: Rect,
    tabs: &[TabItem],
    scroll_offset: usize,
    session_code: &str,
    role: &str,
    status_hint: Option<&str>,
) -> Vec<(Rect, TabStripHit)> {
    let mut hits = Vec::new();
    if area.height == 0 || area.width == 0 || tabs.is_empty() {
        return hits;
    }

    let meta_w = if session_code.is_empty() && role.is_empty() {
        0
    } else {
        right_badge_width(session_code, role, status_hint)
    };

    // Reserve at least space for tabs + '+' when screen is reasonably wide
    let reserved_meta = if area.width > meta_w + 10 { meta_w + 1 } else { 0 };
    let max_x = (area.x + area.width).saturating_sub(reserved_meta);
    let add_w: u16 = 3; // " + "
    let right_arrow_w: u16 = 3; // " > "

    let start_idx = scroll_offset.min(tabs.len().saturating_sub(1));
    let mut cursor_x = area.x;

    // Show left scroll arrow "<" when scrolled past tab 0
    if start_idx > 0 && cursor_x + 3 + add_w <= max_x {
        hits.push((
            Rect {
                x: cursor_x,
                y: area.y,
                width: 3,
                height: 1,
            },
            TabStripHit::ScrollLeft,
        ));
        cursor_x += 3;
    }

    let mut has_more_right = false;
    for (idx, tab) in tabs.iter().enumerate().skip(start_idx) {
        let pill = tab.pill_text();
        let pill_w = pill.chars().count() as u16;
        let has_later_tabs = idx + 1 < tabs.len();
        let needed_after = add_w + if has_later_tabs { right_arrow_w } else { 0 };

        if cursor_x + pill_w + 1 + needed_after <= max_x {
            hits.push((
                Rect {
                    x: cursor_x,
                    y: area.y,
                    width: pill_w,
                    height: 1,
                },
                TabStripHit::Tab(idx),
            ));
            cursor_x += pill_w + 1;
        } else if idx == start_idx && cursor_x + add_w < max_x {
            // Always fit at least the first visible tab clamped cleanly
            let avail = max_x.saturating_sub(cursor_x + add_w + if has_later_tabs { right_arrow_w } else { 0 });
            if avail > 0 {
                hits.push((
                    Rect {
                        x: cursor_x,
                        y: area.y,
                        width: avail,
                        height: 1,
                    },
                    TabStripHit::Tab(idx),
                ));
                cursor_x += avail;
            }
            if has_later_tabs {
                has_more_right = true;
            }
            break;
        } else {
            has_more_right = true;
            break;
        }
    }

    if has_more_right && cursor_x + right_arrow_w + add_w <= max_x {
        hits.push((
            Rect {
                x: cursor_x,
                y: area.y,
                width: right_arrow_w,
                height: 1,
            },
            TabStripHit::ScrollRight,
        ));
        cursor_x += right_arrow_w;
    }

    // '+' add button is ALWAYS pinned cleanly at the end of visible tabs
    if cursor_x + add_w <= area.x + area.width {
        hits.push((
            Rect {
                x: cursor_x,
                y: area.y,
                width: add_w,
                height: 1,
            },
            TabStripHit::AddButton,
        ));
    } else if area.width >= add_w {
        hits.push((
            Rect {
                x: (area.x + area.width).saturating_sub(reserved_meta + add_w),
                y: area.y,
                width: add_w,
                height: 1,
            },
            TabStripHit::AddButton,
        ));
    }

    hits
}

/// Adjust horizontal `scroll_offset` so `active_idx` is always visible in the tab strip.
pub fn ensure_tab_visible(
    area: Rect,
    tabs: &[TabItem],
    active_idx: usize,
    current_scroll: usize,
    session_code: &str,
    role: &str,
    status_hint: Option<&str>,
) -> usize {
    if tabs.is_empty() || area.width == 0 {
        return 0;
    }
    let target = active_idx.min(tabs.len() - 1);
    let mut scroll = current_scroll.min(target);

    for _ in 0..tabs.len() {
        let hits =
            compute_tab_strip_hits_scrolled(area, tabs, scroll, session_code, role, status_hint);
        let visible = hits
            .iter()
            .any(|(_, hit)| matches!(hit, TabStripHit::Tab(i) if *i == target));
        if visible || scroll >= target {
            break;
        }
        scroll += 1;
    }
    scroll
}

/// Render the Herdr-grade Tab Strip into the given area.
pub fn render_tab_strip(
    area: Rect,
    tabs: &[TabItem],
    active_tab_index: usize,
    session_code: &str,
    role: &str,
    _username: &str,
    buf: &mut Buffer,
) {
    render_tab_strip_with_status(
        area,
        tabs,
        active_tab_index,
        0,
        session_code,
        role,
        None,
        buf,
    );
}

/// Render the Herdr-grade Tab Strip with horizontal scrolling and an optional status indicator.
#[allow(clippy::too_many_arguments)]
pub fn render_tab_strip_with_status(
    area: Rect,
    tabs: &[TabItem],
    active_tab_index: usize,
    scroll_offset: usize,
    session_code: &str,
    role: &str,
    status_hint: Option<&str>,
    buf: &mut Buffer,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // 1. Fill entire strip with header background (#181926)
    for x in area.x..(area.x + area.width) {
        let cell = &mut buf[(x, area.y)];
        cell.set_char(' ');
        cell.set_bg(COLOR_HEADER_BG);
        cell.set_style(Theme::tab_strip_bg());
    }

    // 2. Render visible tabs, scroll arrows, and pinned '+' button
    let hits = compute_tab_strip_hits_scrolled(
        area,
        tabs,
        scroll_offset,
        session_code,
        role,
        status_hint,
    );
    for (rect, target) in hits {
        match target {
            TabStripHit::ScrollLeft => {
                let text = " < ";
                for (i, ch) in text.chars().enumerate() {
                    let x = rect.x + i as u16;
                    if x < area.x + area.width {
                        let cell = &mut buf[(x, area.y)];
                        cell.set_char(ch);
                        cell.set_style(Theme::inactive_tab());
                    }
                }
            }
            TabStripHit::ScrollRight => {
                let text = " > ";
                for (i, ch) in text.chars().enumerate() {
                    let x = rect.x + i as u16;
                    if x < area.x + area.width {
                        let cell = &mut buf[(x, area.y)];
                        cell.set_char(ch);
                        cell.set_style(Theme::inactive_tab());
                    }
                }
            }
            TabStripHit::Tab(idx) => {
                let is_active = idx == active_tab_index;
                let pill_style = if is_active {
                    Theme::active_tab_pill()
                } else {
                    Theme::inactive_tab()
                };
                let pill_text = tabs[idx].pill_text();
                for (offset, ch) in pill_text.chars().enumerate() {
                    let x = rect.x + offset as u16;
                    if x >= rect.x + rect.width || x >= area.x + area.width {
                        break;
                    }
                    let cell = &mut buf[(x, area.y)];
                    cell.set_char(ch);
                    cell.set_style(pill_style);
                }
            }
            TabStripHit::AddButton => {
                let plus_x = rect.x + 1;
                if plus_x < area.x + area.width {
                    let cell = &mut buf[(plus_x, area.y)];
                    cell.set_char('+');
                    cell.set_style(Theme::tab_add_button());
                }
            }
        }
    }

    // 3. Render clean right-aligned session badge: `[status]  ● TURF-XXXX • HOST`
    let status_prefix = status_hint
        .map(|s| format!("{s}   "))
        .unwrap_or_default();
    let meta_dot = "● ";
    let meta_code = session_code;
    let meta_sep = " • ";
    let meta_role = role;
    let meta_end = " ";

    let total_meta_len = status_prefix.chars().count()
        + meta_dot.chars().count()
        + meta_code.chars().count()
        + meta_sep.chars().count()
        + meta_role.chars().count()
        + meta_end.chars().count();

    if area.width as usize > total_meta_len + 4 {
        let mut right_x = (area.x + area.width) - total_meta_len as u16;

        let segments: Vec<(&str, Style)> = vec![
            (
                status_prefix.as_str(),
                Style::default().fg(COLOR_GREEN_DOT).bg(COLOR_HEADER_BG),
            ),
            (
                meta_dot,
                Style::default()
                    .fg(COLOR_GREEN_DOT)
                    .bg(COLOR_HEADER_BG),
            ),
            (
                meta_code,
                Style::default()
                    .fg(COLOR_TEXT_PRIMARY)
                    .bg(COLOR_HEADER_BG)
                    .add_modifier(Modifier::BOLD),
            ),
            (
                meta_sep,
                Style::default().fg(COLOR_TEXT_DIM).bg(COLOR_HEADER_BG),
            ),
            (
                meta_role,
                Style::default()
                    .fg(COLOR_TEXT_MUTED)
                    .bg(COLOR_HEADER_BG)
                    .add_modifier(Modifier::BOLD),
            ),
            (
                meta_end,
                Style::default().fg(COLOR_TEXT_DIM).bg(COLOR_HEADER_BG),
            ),
        ];

        for (text, style) in segments {
            for ch in text.chars() {
                if right_x < area.x + area.width {
                    let cell = &mut buf[(right_x, area.y)];
                    cell.set_char(ch);
                    cell.set_style(style);
                    right_x += 1;
                }
            }
        }
    }
}
