#[derive(Debug, Clone)]
pub struct InputState {
    pub text: String,
    pub cursor_pos: usize,
    pub history: Vec<String>,
    history_index: Option<usize>,
    saved_input: Option<String>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_index: None,
            saved_input: None,
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn submit(&mut self) -> String {
        let cmd = self.text.clone();
        if !cmd.trim().is_empty() {
            self.history.push(cmd.clone());
        }
        self.text.clear();
        self.cursor_pos = 0;
        self.history_index = None;
        self.saved_input = None;
        cmd
    }

    pub fn insert_char(&mut self, c: char) {
        self.text.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        self.text.insert(self.cursor_pos, '\n');
        self.cursor_pos += 1;
    }

    pub fn insert_text(&mut self, text: &str) {
        for c in text.chars() {
            if c == '\r' {
                continue;
            }
            if c == '\n' {
                self.insert_newline();
            } else {
                self.insert_char(c);
            }
        }
    }

    pub fn delete_before_cursor(&mut self) {
        if self.cursor_pos > 0 {
            if let Some(prev) = self.prev_char_boundary() {
                self.text.replace_range(prev..self.cursor_pos, "");
                self.cursor_pos = prev;
            }
        }
    }

    pub fn delete_at_cursor(&mut self) {
        if self.cursor_pos < self.text.len() {
            if let Some(next) = self.next_char_boundary() {
                self.text.replace_range(self.cursor_pos..next, "");
            }
        }
    }

    fn prev_char_boundary(&self) -> Option<usize> {
        if self.cursor_pos == 0 {
            return None;
        }
        let mut pos = self.cursor_pos - 1;
        while pos > 0 && !self.text.is_char_boundary(pos) {
            pos -= 1;
        }
        Some(pos)
    }

    fn next_char_boundary(&self) -> Option<usize> {
        if self.cursor_pos >= self.text.len() {
            return None;
        }
        let mut pos = self.cursor_pos + 1;
        while pos < self.text.len() && !self.text.is_char_boundary(pos) {
            pos += 1;
        }
        Some(pos)
    }

    pub fn move_cursor_left(&mut self) {
        if let Some(prev) = self.prev_char_boundary() {
            self.cursor_pos = prev;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if let Some(next) = self.next_char_boundary() {
            self.cursor_pos = next;
        }
    }

    pub fn move_cursor_up(&mut self) {
        let (line, col) = self.cursor_line_col();
        if line == 0 {
            self.cursor_pos = 0;
            return;
        }
        let target_line = line.saturating_sub(1);
        let target_col = col;
        self.cursor_pos = self.pos_from_line_col(target_line, target_col);
    }

    pub fn move_cursor_down(&mut self) {
        let (line, col) = self.cursor_line_col();
        let total_lines = self.line_count();
        if line >= total_lines.saturating_sub(1) {
            self.cursor_pos = self.text.len();
            return;
        }
        let target_line = line + 1;
        let target_col = col;
        self.cursor_pos = self.pos_from_line_col(target_line, target_col);
    }

    pub fn move_cursor_home(&mut self) {
        let (line, _) = self.cursor_line_col();
        self.cursor_pos = self.pos_from_line_col(line, 0);
    }

    pub fn move_cursor_end(&mut self) {
        let (line, _) = self.cursor_line_col();
        let line_end = self.line_end_pos(line);
        self.cursor_pos = line_end;
    }

    pub fn delete_word_before(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let chars: Vec<(usize, char)> = self.text.char_indices().collect();

        let mut idx = chars
            .iter()
            .position(|(i, _)| *i == self.cursor_pos)
            .unwrap_or(chars.len());

        while idx > 0 {
            let (_, c) = chars[idx - 1];
            if c.is_whitespace() {
                idx -= 1;
            } else {
                break;
            }
        }
        while idx > 0 {
            let (_, c) = chars[idx - 1];
            if c.is_whitespace() {
                break;
            }
            idx -= 1;
        }

        let pos = if idx < chars.len() {
            chars[idx].0
        } else {
            self.text.len()
        };

        self.text.replace_range(pos..self.cursor_pos, "");
        self.cursor_pos = pos;
    }

    pub fn cursor_line_col(&self) -> (usize, usize) {
        let mut line = 0;
        let mut col = 0;
        for (i, c) in self.text.char_indices() {
            if i >= self.cursor_pos {
                break;
            }
            if c == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn pos_from_line_col(&self, target_line: usize, target_col: usize) -> usize {
        let mut line = 0;
        let mut col = 0;
        for (i, c) in self.text.char_indices() {
            if line == target_line && col == target_col {
                return i;
            }
            if c == '\n' {
                line += 1;
                col = 0;
                if line > target_line {
                    return i; // target line doesn't exist yet, return start of this line
                }
            } else {
                col += 1;
            }
        }
        self.text.len()
    }

    fn line_end_pos(&self, target_line: usize) -> usize {
        let mut line = 0;
        for (i, c) in self.text.char_indices() {
            if c == '\n' {
                if line == target_line {
                    return i;
                }
                line += 1;
            }
        }
        self.text.len()
    }

    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            return 1;
        }
        self.text.chars().filter(|&c| c == '\n').count() + 1
    }

    pub fn cursor_line(&self) -> usize {
        self.cursor_line_col().0
    }

    pub fn cursor_col(&self) -> usize {
        self.cursor_line_col().1
    }

    pub fn lines(&self) -> Vec<&str> {
        if self.text.is_empty() {
            return vec![""];
        }
        self.text.split('\n').collect()
    }

    pub(crate) fn reset_history_nav(&mut self) {
        self.history_index = None;
        self.saved_input = None;
    }

    fn save_current(&mut self) {
        if self.saved_input.is_none() {
            self.saved_input = Some(self.text.clone());
        }
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        self.save_current();
        match self.history_index {
            None => {
                self.history_index = Some(self.history.len() - 1);
            }
            Some(0) => {}
            Some(i) => {
                self.history_index = Some(i - 1);
            }
        }
        if let Some(idx) = self.history_index {
            self.text = self.history[idx].clone();
            self.cursor_pos = self.text.len();
        }
    }

    pub fn history_next(&mut self) {
        match self.history_index {
            None => return,
            Some(i) if i >= self.history.len() - 1 => {
                self.history_index = None;
                self.text = self.saved_input.take().unwrap_or_default();
                self.cursor_pos = self.text.len();
                self.saved_input = None;
            }
            Some(i) => {
                self.history_index = Some(i + 1);
                self.text = self.history[i + 1].clone();
                self.cursor_pos = self.text.len();
            }
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}
