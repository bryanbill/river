use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub header_bg: Color,
    pub header_fg: Color,
    pub output_text: Color,
    pub output_dim: Color,
    pub output_error: Color,
    pub output_json_key: Color,
    pub output_json_string: Color,
    pub output_json_number: Color,
    pub output_json_bool: Color,
    pub output_json_null: Color,
    pub output_json_brace: Color,
    pub table_header_bg: Color,
    pub table_header_fg: Color,
    pub table_border: Color,
    pub table_row_alt: Color,
    pub input_prefix: Color,
    pub input_text: Color,
    pub input_cursor: Color,
    pub separator: Color,
    pub syntax_keyword: Color,
    pub syntax_function: Color,
    pub syntax_string: Color,
    pub syntax_number: Color,
    pub syntax_operator: Color,
    pub syntax_parameter: Color,
    pub syntax_identifier: Color,
    pub syntax_punctuation: Color,
    pub syntax_bool: Color,
    pub syntax_null: Color,
    pub scrollbar_track: Color,
    pub scrollbar_thumb: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            header_bg: Color::Rgb(40, 44, 52),
            header_fg: Color::Rgb(152, 195, 121),
            output_text: Color::Rgb(171, 178, 191),
            output_dim: Color::Rgb(92, 99, 112),
            output_error: Color::Rgb(224, 108, 117),
            output_json_key: Color::Rgb(224, 108, 117),
            output_json_string: Color::Rgb(152, 195, 121),
            output_json_number: Color::Rgb(209, 154, 102),
            output_json_bool: Color::Rgb(86, 182, 194),
            output_json_null: Color::Rgb(92, 99, 112),
            output_json_brace: Color::Rgb(171, 178, 191),
            table_header_bg: Color::Rgb(40, 44, 52),
            table_header_fg: Color::Rgb(229, 192, 123),
            table_border: Color::Rgb(92, 99, 112),
            table_row_alt: Color::Rgb(33, 37, 43),
            input_prefix: Color::Rgb(152, 195, 121),
            input_text: Color::Rgb(171, 178, 191),
            input_cursor: Color::Rgb(97, 175, 239),
            separator: Color::Rgb(92, 99, 112),
            syntax_keyword: Color::Rgb(198, 120, 221),
            syntax_function: Color::Rgb(86, 182, 194),
            syntax_string: Color::Rgb(152, 195, 121),
            syntax_number: Color::Rgb(209, 154, 102),
            syntax_operator: Color::Rgb(86, 182, 194),
            syntax_parameter: Color::Rgb(229, 192, 123),
            syntax_identifier: Color::Rgb(171, 178, 191),
            syntax_punctuation: Color::Rgb(92, 99, 112),
            syntax_bool: Color::Rgb(86, 182, 194),
            syntax_null: Color::Rgb(92, 99, 112),
            scrollbar_track: Color::Rgb(60, 63, 71),
            scrollbar_thumb: Color::Rgb(92, 99, 112),
        }
    }

    pub fn header_style(&self) -> Style {
        Style::default().fg(self.header_fg).bg(self.header_bg)
    }

    pub fn output_info_style(&self) -> Style {
        Style::default().fg(self.output_dim)
    }

    pub fn output_error_style(&self) -> Style {
        Style::default().fg(self.output_error)
    }

    pub fn output_text_style(&self) -> Style {
        Style::default().fg(self.output_text)
    }

    pub fn input_prefix_style(&self) -> Style {
        Style::default().fg(self.input_prefix)
    }

    pub fn input_cursor_style(&self) -> Style {
        Style::default()
            .fg(Color::Black)
            .bg(self.input_cursor)
            .add_modifier(Modifier::BOLD)
    }

    pub fn input_cursor_overlay(&self, base: Style) -> Style {
        base.bg(self.input_cursor)
            .add_modifier(Modifier::BOLD)
    }

    pub fn table_header_style(&self) -> Style {
        Style::default()
            .fg(self.table_header_fg)
            .bg(self.table_header_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn separator_style(&self) -> Style {
        Style::default().fg(self.separator)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
