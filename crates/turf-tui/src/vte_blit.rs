//! High-performance, zero-flicker cell blitter from `turf_vte::ScreenBuffer`
//! directly into Ratatui's `Buffer`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color as RatColor, Modifier, Style};
use turf_vte::{Cell, CellFlags, Color as VteColor, NamedColor, ScreenBuffer};

use crate::theme::COLOR_TEXT_PRIMARY;

/// Convert a `turf_vte::Color` into a `ratatui::style::Color`.
pub fn vte_color_to_ratatui(color: VteColor, is_bg: bool) -> RatColor {
    match color {
        VteColor::Default => {
            if is_bg {
                RatColor::Reset
            } else {
                COLOR_TEXT_PRIMARY
            }
        }
        VteColor::Named(named) => match named {
            NamedColor::Black => RatColor::Black,
            NamedColor::Red => RatColor::Red,
            NamedColor::Green => RatColor::Green,
            NamedColor::Yellow => RatColor::Yellow,
            NamedColor::Blue => RatColor::Blue,
            NamedColor::Magenta => RatColor::Magenta,
            NamedColor::Cyan => RatColor::Cyan,
            NamedColor::White => RatColor::Gray,
            NamedColor::BrightBlack => RatColor::DarkGray,
            NamedColor::BrightRed => RatColor::LightRed,
            NamedColor::BrightGreen => RatColor::LightGreen,
            NamedColor::BrightYellow => RatColor::LightYellow,
            NamedColor::BrightBlue => RatColor::LightBlue,
            NamedColor::BrightMagenta => RatColor::LightMagenta,
            NamedColor::BrightCyan => RatColor::LightCyan,
            NamedColor::BrightWhite => RatColor::White,
        },
        VteColor::Indexed(idx) => RatColor::Indexed(idx),
        VteColor::Rgb(r, g, b) => RatColor::Rgb(r, g, b),
    }
}

/// Convert `turf_vte::CellFlags` into `ratatui::style::Modifier`.
pub fn vte_flags_to_modifiers(flags: CellFlags) -> Modifier {
    let mut modifier = Modifier::empty();
    if flags.bold {
        modifier |= Modifier::BOLD;
    }
    if flags.dim {
        modifier |= Modifier::DIM;
    }
    if flags.italic {
        modifier |= Modifier::ITALIC;
    }
    if flags.underline {
        modifier |= Modifier::UNDERLINED;
    }
    if flags.inverse {
        modifier |= Modifier::REVERSED;
    }
    if flags.strikethrough {
        modifier |= Modifier::CROSSED_OUT;
    }
    if flags.blink {
        modifier |= Modifier::SLOW_BLINK;
    }
    if flags.hidden {
        modifier |= Modifier::HIDDEN;
    }
    modifier
}

/// Selection span `(start_col, start_row)` to `(end_col, end_row)` in pane-local coordinates.
pub type PaneSelection = ((u16, u16), (u16, u16));

fn is_cell_selected(x: u16, y: u16, sel: PaneSelection) -> bool {
    let ((sx, sy), (ex, ey)) = if (sel.0 .1, sel.0 .0) <= (sel.1 .1, sel.1 .0) {
        (sel.0, sel.1)
    } else {
        (sel.1, sel.0)
    };
    if y < sy || y > ey {
        return false;
    }
    if sy == ey {
        x >= sx && x <= ex
    } else if y == sy {
        x >= sx
    } else if y == ey {
        x <= ex
    } else {
        true
    }
}

/// Blit an entire `ScreenBuffer` directly into a Ratatui `Buffer` slice bounded by `area`.
pub fn blit_screen_buffer(
    screen: &ScreenBuffer,
    area: Rect,
    buf: &mut Buffer,
    cursor: Option<(usize, usize)>,
    is_focused: bool,
) {
    blit_screen_buffer_with_selection(screen, area, buf, cursor, is_focused, None);
}

/// Blit an entire `ScreenBuffer` into a Ratatui `Buffer` slice with optional pane-local highlight selection.
pub fn blit_screen_buffer_with_selection(
    screen: &ScreenBuffer,
    area: Rect,
    buf: &mut Buffer,
    cursor: Option<(usize, usize)>,
    is_focused: bool,
    selection: Option<PaneSelection>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let rows = (area.height as usize).min(screen.rows());
    let cols = (area.width as usize).min(screen.cols());
    let viewport = screen.render_viewport_rows(screen.scroll_offset(), rows);
    let is_scrolled_back = screen.scroll_offset() > 0;
    let selection_bg = RatColor::Rgb(69, 71, 90);

    for (y, row) in viewport.iter().take(rows).enumerate() {
        let dest_y = area.y + y as u16;
        for x in 0..cols {
            let dest_x = area.x + x as u16;

            let cell: &Cell = match row.get(x) {
                Some(c) => c,
                None => continue,
            };

            // Trailing continuation cell of a 2-column glyph
            if cell.c == '\0' {
                let dest_cell = &mut buf[(dest_x, dest_y)];
                dest_cell.set_char(' ');
                dest_cell.set_skip(true);
                continue;
            }

            let c = if cell.c == ' ' { ' ' } else { cell.c };

            let mut fg = vte_color_to_ratatui(cell.fg, false);
            let mut bg = vte_color_to_ratatui(cell.bg, true);
            let mut modifier = vte_flags_to_modifiers(cell.flags);

            // Highlight clickable OSC 8 hyperlinks with subtle underline when not already underlined
            if cell.hyperlink.is_some() {
                modifier |= Modifier::UNDERLINED;
            }

            // Apply pane-bounded selection highlight cleanly without bleeding across dividers
            if let Some(sel) = selection {
                if is_cell_selected(x as u16, y as u16, sel) {
                    bg = selection_bg;
                }
            }

            // Render clean cursor using reverse video without neon green artifacts
            if let Some((cx, cy)) = cursor {
                let clamped_cx = cx.min(cols.saturating_sub(1));
                if !is_scrolled_back && clamped_cx == x && cy == y && is_focused {
                    modifier |= Modifier::REVERSED;
                    if c == ' ' && (fg == RatColor::Reset || fg == bg) {
                        fg = COLOR_TEXT_PRIMARY;
                    }
                }
            }

            let dest_cell = &mut buf[(dest_x, dest_y)];
            dest_cell.set_skip(false);
            dest_cell.set_char(c);
            dest_cell.set_fg(fg);
            dest_cell.set_bg(bg);
            dest_cell.set_style(Style::default().add_modifier(modifier));
        }

        // Blank out any remaining trailing columns with Reset background
        for x in cols..(area.width as usize) {
            let dest_x = area.x + x as u16;
            let dest_cell = &mut buf[(dest_x, dest_y)];
            dest_cell.set_skip(false);
            dest_cell.set_char(' ');
            dest_cell.set_bg(RatColor::Reset);
            dest_cell.set_fg(COLOR_TEXT_PRIMARY);
            dest_cell.set_style(Style::default());
        }
    }

    // Blank out any remaining trailing rows with Reset background
    for y in rows..(area.height as usize) {
        let dest_y = area.y + y as u16;
        for x in 0..area.width {
            let dest_x = area.x + x;
            let dest_cell = &mut buf[(dest_x, dest_y)];
            dest_cell.set_skip(false);
            dest_cell.set_char(' ');
            dest_cell.set_bg(RatColor::Reset);
            dest_cell.set_fg(COLOR_TEXT_PRIMARY);
            dest_cell.set_style(Style::default());
        }
    }
}
