use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::tui::app::{App, Status};
use crate::tui::highlight;
use crate::tui::output::OutputLine;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = frame.area();

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(input_height(app, area)),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(frame, main_layout[0], app, theme);
    render_output(frame, main_layout[1], app, theme);
    render_input(frame, main_layout[2], app, theme);
    render_status_bar(frame, main_layout[3], app, theme);

    if app.show_help {
        render_help_overlay(frame, area, theme);
    }
}

fn input_height(app: &App, area: Rect) -> u16 {
    let lines = app.input.line_count() as u16;
    let needed = lines.saturating_add(1);
    let reserved = 10u16;
    let max = area.height.saturating_sub(reserved).max(4);
    let min = 3u16;
    needed.clamp(min, max)
}

fn render_header(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let conn_info = app
        .active_connection
        .as_ref()
        .map(|c| c.as_str())
        .unwrap_or("no connection");

    let status_indicator = match app.status {
        Status::Running => Span::styled(
            " ⏳ Running... ",
            theme
                .header_style()
                .fg(theme.output_json_number)
                .add_modifier(Modifier::BOLD),
        ),
        Status::Error(_) => Span::styled(
            " ✗ ",
            theme.header_style().fg(theme.output_error),
        ),
        Status::Idle => Span::styled(
            " ✓ ",
            theme.header_style().fg(theme.header_fg),
        ),
    };

    let text = Line::from(vec![
        Span::styled(" River ", theme.header_style().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" {} ", conn_info),
            theme
                .header_style()
                .fg(theme.output_dim)
                .add_modifier(Modifier::ITALIC),
        ),
        status_indicator,
    ]);

    frame.render_widget(
        Paragraph::new(text).block(Block::default().style(theme.header_style())),
        area,
    );
}

fn render_output(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let visible_height = area.height as usize;
    let (offset, visible_lines) = app.output.visible_range(visible_height);

    let scroll_hint = if offset > 0 {
        format!(
            " [↑ {} more lines — PgUp to scroll] ",
            offset
        )
    } else {
        String::new()
    };

    let lines: Vec<Line> = visible_lines
        .iter()
        .flat_map(|line| render_output_line(line, area.width, visible_height, theme))
        .collect();

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(
                Block::default()
                    .borders(Borders::NONE)
                    .title_top(scroll_hint.dim()),
            ),
        area,
    );
}

fn render_output_line<'a>(line: &'a OutputLine, max_width: u16, available_height: usize, theme: &'a Theme) -> Vec<Line<'a>> {
    match line {
        OutputLine::Info(text) => vec![Line::from(Span::styled(
            text.as_str(),
            theme.output_info_style(),
        ))],
        OutputLine::Error(text) => {
            let indicator = Span::styled(
                "✗ ",
                Style::default()
                    .fg(Color::White)
                    .bg(theme.output_error)
                    .add_modifier(Modifier::BOLD),
            );
            let msg = Span::styled(text.as_str(), theme.output_error_style());
            vec![Line::from(vec![indicator, msg])]
        }
        OutputLine::Separator => {
            let width = max_width as usize;
            vec![Line::from(Span::styled(
                "─".repeat(width),
                theme.separator_style(),
            ))]
        }
        OutputLine::Json(json) => {
            vec![Line::from(highlight_json(json, theme))]
        }
        OutputLine::Table { headers, rows, row_offset } => {
            render_compact_table(headers, rows, *row_offset, max_width, available_height, theme)
        }
    }
}

fn render_compact_table<'a>(
    headers: &[String],
    rows: &[Vec<String>],
    row_offset: usize,
    max_width: u16,
    available_height: usize,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();

    let col_count = headers.len();
    if col_count == 0 {
        lines.push(Line::from("(empty table)"));
        return lines;
    }

    if rows.is_empty() {
        lines.push(Line::from("(empty table)"));
        return lines;
    }

    let total_rows = rows.len();
    let max_display = crate::tui::output::MAX_TABLE_DISPLAY_ROWS;
    let cap = max_display.min(available_height.saturating_sub(6));
    let cap = cap.max(1);

    let start = row_offset.min(total_rows.saturating_sub(1));
    let end = (start + cap).min(total_rows);
    let slice = &rows[start..end];

    let has_more_above = start > 0;
    let has_more_below = end < total_rows;

    let mut col_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in slice {
        for (i, cell) in row.iter().enumerate() {
            if i < col_widths.len() {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }
    }

    let max_col = 40usize;
    for w in &mut col_widths {
        *w = (*w).min(max_col).max(3);
    }

    let total: usize = col_widths.iter().sum();
    let available = max_width.saturating_sub(3) as usize;
    let effective = total.min(available);
    let bar_width = effective + col_widths.len().saturating_mul(3).saturating_sub(1);

    let h_style = theme.table_header_style();
    let border_fg = theme.table_border;

    let bar = |c1: char, c2: char| -> Line<'a> {
        Line::from(Span::styled(
            format!("{}{}{}", c1, "─".repeat(bar_width), c2),
            Style::default().fg(border_fg),
        ))
    };

    if has_more_above {
        lines.push(Line::from(Span::styled(
            format!("  ... {} more row(s) above (press Up to scroll) ...", start),
            theme.output_info_style(),
        )));
    }

    lines.push(bar('┌', '┐'));

    let mut header_spans: Vec<Span> = vec![Span::styled("│ ", Style::default().fg(border_fg))];
    for (i, h) in headers.iter().enumerate() {
        let w = col_widths[i];
        let padded = format!("{:w$}", h, w = w);
        header_spans.push(Span::styled(padded, h_style));
        if i < headers.len() - 1 {
            header_spans.push(Span::styled(" │ ", Style::default().fg(border_fg)));
        }
    }
    header_spans.push(Span::styled(" │", Style::default().fg(border_fg)));
    lines.push(Line::from(header_spans));

    lines.push(bar('├', '┤'));

    for (ri, row) in slice.iter().enumerate() {
        let row_style = if ri % 2 == 0 {
            theme.output_text_style()
        } else {
            Style::default()
                .fg(theme.output_text)
                .bg(theme.table_row_alt)
        };

        let mut row_spans: Vec<Span> = vec![Span::styled("│ ", Style::default().fg(border_fg))];
        for (i, cell) in row.iter().enumerate() {
            let w = col_widths[i];
            let padded = format!("{:w$}", cell, w = w);
            row_spans.push(Span::styled(padded, row_style));
            if i < row.len() - 1 {
                row_spans.push(Span::styled(" │ ", Style::default().fg(border_fg)));
            }
        }
        row_spans.push(Span::styled(" │", Style::default().fg(border_fg)));
        lines.push(Line::from(row_spans));
    }

    lines.push(bar('└', '┘'));

    if has_more_below {
        let remaining = total_rows - end;
        lines.push(Line::from(Span::styled(
            format!("  ... {} more row(s) below (press Down to scroll) ...", remaining),
            theme.output_info_style(),
        )));
    }

    lines
}

fn highlight_json<'a>(json: &'a str, theme: &'a Theme) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let chars: Vec<char> = json.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        match c {
            '{' | '}' | '[' | ']' | ',' | ':' => {
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default().fg(theme.output_json_brace),
                ));
                i += 1;
            }
            '"' => {
                let mut s = String::new();
                s.push('"');
                i += 1;
                while i < len {
                    if chars[i] == '\\' && i + 1 < len {
                        s.push(chars[i]);
                        s.push(chars[i + 1]);
                        i += 2;
                    } else if chars[i] == '"' {
                        s.push('"');
                        i += 1;
                        break;
                    } else {
                        s.push(chars[i]);
                        i += 1;
                    }
                }
                let mut j = i;
                while j < len && (chars[j] == ' ' || chars[j] == '\n' || chars[j] == '\t') {
                    j += 1;
                }
                let is_key = j < len && chars[j] == ':';
                let style = if is_key {
                    Style::default().fg(theme.output_json_key)
                } else {
                    Style::default().fg(theme.output_json_string)
                };
                spans.push(Span::styled(s, style));
            }
            't' | 'f' if looks_like_bool(&chars, i) => {
                let word = if chars[i] == 't' { "true" } else { "false" };
                i += word.len();
                spans.push(Span::styled(
                    word,
                    Style::default().fg(theme.output_json_bool),
                ));
            }
            'n' if looks_like_null(&chars, i) => {
                spans.push(Span::styled(
                    "null",
                    Style::default().fg(theme.output_json_null),
                ));
                i += 4;
            }
            '-' | '0'..='9' => {
                let num: String = chars[i..]
                    .iter()
                    .take_while(|c| matches!(c, '-' | '0'..='9' | '.' | 'e' | 'E' | '+'))
                    .collect();
                let len = num.len();
                spans.push(Span::styled(
                    num,
                    Style::default().fg(theme.output_json_number),
                ));
                i += len;
            }
            _ => {
                let mut s = String::new();
                while i < len
                    && !matches!(
                        chars[i],
                        '{' | '}' | '[' | ']' | ',' | ':' | '"' | '-' | '0'..='9'
                    )
                {
                    if looks_like_bool(&chars, i) || looks_like_null(&chars, i) {
                        break;
                    }
                    s.push(chars[i]);
                    i += 1;
                }
                if !s.is_empty() {
                    spans.push(Span::styled(s, Style::default().fg(theme.output_text)));
                }
            }
        }
    }

    spans
}

fn looks_like_bool(chars: &[char], i: usize) -> bool {
    chars.get(i..).map_or(false, |s| {
        (s.len() >= 4 && s[0] == 't' && s[1] == 'r' && s[2] == 'u' && s[3] == 'e')
            || (s.len() >= 5
                && s[0] == 'f'
                && s[1] == 'a'
                && s[2] == 'l'
                && s[3] == 's'
                && s[4] == 'e')
    })
}

fn looks_like_null(chars: &[char], i: usize) -> bool {
    chars
        .get(i..)
        .is_some_and(|s| s.len() >= 4 && s[0] == 'n' && s[1] == 'u' && s[2] == 'l' && s[3] == 'l')
}

fn render_input(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let all_lines = app.input.lines();
    let cursor_line = app.input.cursor_line();
    let cursor_col = app.input.cursor_col();

    let tokens = highlight::tokenize(&app.input.text);

    let border_height = 1u16;
    let area_height = (area.height as usize).saturating_sub(border_height as usize);

    let total_lines = all_lines.len();

    let (view_offset, content_height) = if total_lines <= area_height {
        (0, total_lines)
    } else {
        let ch = area_height.saturating_sub(2).max(1);
        let off = if cursor_line < ch / 2 {
            0
        } else if cursor_line >= total_lines.saturating_sub(ch / 2) {
            total_lines.saturating_sub(ch)
        } else {
            cursor_line.saturating_sub(ch / 2)
        };
        (off, ch)
    };

    let view_end = (view_offset + content_height).min(total_lines);

    let has_more_above = view_offset > 0;
    let has_more_below = view_end < total_lines;

    let mut rendered_lines: Vec<Line> = Vec::with_capacity(area_height);

    if has_more_above {
        rendered_lines.push(Line::from(Span::styled(
            format!("  ↑ {} more line(s) above", view_offset),
            Style::default().fg(theme.output_dim),
        )));
    }

    for (line_idx, line_text) in all_lines.iter().enumerate().skip(view_offset).take(view_end - view_offset) {
        let line_start = highlight::line_start_byte(&app.input.text, line_idx);

        let is_cursor_line = line_idx == cursor_line;
        let cursor_byte = if is_cursor_line {
            Some(
                line_text
                    .chars()
                    .take(cursor_col)
                    .map(|c| c.len_utf8())
                    .sum(),
            )
        } else {
            None
        };

        let spans =
            highlight::highlight_line(line_text, line_start, &tokens, cursor_byte, theme);

        let prefix = if is_cursor_line {
            Span::styled("> ", theme.input_prefix_style())
        } else {
            Span::styled("  ", Style::default().fg(theme.output_dim))
        };

        let mut line_spans = vec![prefix];
        line_spans.extend(spans);
        rendered_lines.push(Line::from(line_spans));
    }

    if has_more_below {
        let remaining = total_lines - view_end;
        rendered_lines.push(Line::from(Span::styled(
            format!("  ↓ {} more line(s) below", remaining),
            Style::default().fg(theme.output_dim),
        )));
    }

    if rendered_lines.is_empty() {
        let prefix = Span::styled("> ", theme.input_prefix_style());
        let cursor = Span::styled(" ", theme.input_cursor_style());
        rendered_lines.push(Line::from(vec![prefix, cursor]));
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(theme.separator_style());

    frame.render_widget(
        Paragraph::new(Text::from(rendered_lines)).block(block),
        area,
    );
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let db_count = app.adapters.len();

    let mut spans: Vec<Span> = vec![
        Span::styled("Enter: submit", theme.status_bar_style()),
        Span::styled(" | ", theme.status_bar_style()),
        Span::styled("S-Enter/Alt-Enter: newline", theme.status_bar_style()),
        Span::styled(" | ", theme.status_bar_style()),
        Span::styled("↑↓: move cursor", theme.status_bar_style()),
        Span::styled(" | ", theme.status_bar_style()),
        Span::styled("C-↑↓: history", theme.status_bar_style()),
        Span::styled(" | ", theme.status_bar_style()),
        Span::styled("S-↑↓: scroll table", theme.status_bar_style()),
        Span::styled(" | ", theme.status_bar_style()),
        Span::styled("PgUp/Dn: page", theme.status_bar_style()),
        Span::styled(" | ", theme.status_bar_style()),
        Span::styled("Ctrl+P/N: input hist", theme.status_bar_style()),
        Span::styled(" | ", theme.status_bar_style()),
        Span::styled("Ctrl+D: quit", theme.status_bar_style()),
        Span::styled(" | ", theme.status_bar_style()),
        Span::styled("?: help", theme.status_bar_style()),
        Span::styled(" | ", theme.status_bar_style()),
        Span::styled(format!("{} db(s)", db_count), theme.status_bar_style()),
    ];

    if let Status::Error(msg) = &app.status {
        spans.push(Span::styled(
            format!(" ✗ {}", msg),
            Style::default()
                .fg(Color::White)
                .bg(theme.output_error),
        ));
    }

    if matches!(app.status, Status::Running) {
        spans.push(Span::styled(
            " Running... ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let bar = Line::from(spans);

    frame.render_widget(
        Paragraph::new(bar).block(Block::default().style(theme.status_bar_style())),
        area,
    );
}

fn render_help_overlay(frame: &mut Frame, area: Rect, theme: &Theme) {
    let overlay_width = 54u16;
    let overlay_height = 26u16;

    let x = area.width.saturating_sub(overlay_width) / 2;
    let y = area.height.saturating_sub(overlay_height) / 2;

    let overlay_area = Rect {
        x: area.x + x,
        y: area.y + y,
        width: overlay_width.min(area.width),
        height: overlay_height.min(area.height),
    };

    let help_text = vec![
        Line::from(Span::styled(
            " Keybindings ",
            Style::default()
                .fg(theme.header_fg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  Enter          Submit query"),
        Line::from("  Shift+Enter    Newline (kitty/wezterm)"),
        Line::from("  Alt+Enter      Newline (reliable fallback)"),
        Line::from("  Ctrl+J         Newline"),
        Line::from("  Paste          Multi-line paste"),
        Line::from("  Up/Down        Move cursor within input"),
        Line::from("  Ctrl+Up/Down   Navigate query history"),
        Line::from("  Shift+Up/Down  Scroll table rows"),
        Line::from("  PgUp/PgDown    Scroll output page"),
        Line::from("  Ctrl+P/N       Input history prev/next"),
        Line::from("  Ctrl+W         Delete word before cursor"),
        Line::from("  Ctrl+D         Quit"),
        Line::from("  Ctrl+L         Clear output"),
        Line::from("  Ctrl+C         Cancel running query"),
        Line::from("  Left/Right     Move cursor in input"),
        Line::from("  Home/End       Jump to line start/end"),
        Line::from("  / or ?         Show this help"),
        Line::from("  :help          Show this help"),
        Line::from("  :quit          Quit"),
        Line::from(""),
        Line::from(Span::styled(
            " Commands ",
            Style::default()
                .fg(theme.header_fg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  find ...      Query data"),
        Line::from("  show tables   List tables"),
        Line::from("  describe <t>  Describe table"),
        Line::from(""),
        Line::from(Span::styled(
            " Press any key to close ",
            Style::default().fg(theme.output_dim),
        )),
    ];

    frame.render_widget(Clear, overlay_area);
    frame.render_widget(
        Paragraph::new(Text::from(help_text))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.table_border))
                    .style(
                        Style::default()
                            .bg(theme.table_header_bg)
                            .fg(theme.output_text),
                    ),
            ),
        overlay_area,
    );
}
