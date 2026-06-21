#![allow(dead_code)]

use std::collections::VecDeque;

pub const MAX_TABLE_DISPLAY_ROWS: usize = 50;

#[derive(Debug, Clone)]
pub enum OutputLine {
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        row_offset: usize,
    },
    Json(String),
    Error(String),
    Info(String),
    Separator,
}

impl OutputLine {
    fn table_overhead(&self) -> usize {
        match self {
            OutputLine::Table { rows, row_offset, .. } => {
                if rows.is_empty() {
                    return 0;
                }
                let total = rows.len();
                let display = MAX_TABLE_DISPLAY_ROWS.min(rows.len().saturating_sub(*row_offset));
                let has_more_above = *row_offset > 0;
                let has_more_below = row_offset + display < total;
                let mut extra = 0usize;
                if has_more_above { extra += 1; }
                if has_more_below { extra += 1; }
                4 + extra // top, header, sep, bottom + truncation lines
            }
            _ => 0,
        }
    }

    pub fn visual_height(&self) -> usize {
        match self {
            OutputLine::Info(_) | OutputLine::Error(_) | OutputLine::Separator => 1,
            OutputLine::Json(json) => 1 + json.lines().count().saturating_sub(1),
            OutputLine::Table { rows, row_offset, .. } => {
                if rows.is_empty() {
                    return 1;
                }
                let total = rows.len();
                let visible = MAX_TABLE_DISPLAY_ROWS.min(total.saturating_sub(*row_offset));
                self.table_overhead() + visible
            }
        }
    }

    pub fn total_rows(&self) -> usize {
        match self {
            OutputLine::Table { rows, .. } => rows.len(),
            _ => 0,
        }
    }

    pub fn set_row_offset(&mut self, offset: usize) {
        if let OutputLine::Table { row_offset, rows, .. } = self {
            let max = rows.len().saturating_sub(1);
            *row_offset = offset.min(max);
        }
    }

    pub fn row_offset(&self) -> usize {
        match self {
            OutputLine::Table { row_offset, .. } => *row_offset,
            _ => 0,
        }
    }
}

pub struct OutputBuffer {
    lines: VecDeque<OutputLine>,
    max_items: usize,
    scroll_offset: usize,
}

impl OutputBuffer {
    pub fn new(max_items: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(max_items),
            max_items,
            scroll_offset: 0,
        }
    }

    pub fn push(&mut self, line: OutputLine) {
        while self.lines.len() >= self.max_items {
            self.lines.pop_front();
            if self.scroll_offset > 0 {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
        }
        self.lines.push_back(line);
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &OutputLine> {
        self.lines.iter()
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    fn total_visual_lines(&self) -> usize {
        self.lines.iter().map(|l| l.visual_height()).sum()
    }

    pub fn scroll_up(&mut self, amount: usize) {
        let total = self.total_visual_lines();
        let max_offset = total.saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + amount).min(max_offset);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_page_up(&mut self, page_size: usize) {
        self.scroll_up(page_size);
    }

    pub fn scroll_page_down(&mut self, page_size: usize) {
        self.scroll_down(page_size);
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_offset = 0;
    }

    fn last_table_mut(&mut self) -> Option<&mut OutputLine> {
        self.lines.iter_mut().rev().find(|line| {
            matches!(line, OutputLine::Table { .. })
        })
    }

    pub fn scroll_last_table_up(&mut self, amount: usize) {
        if let Some(table) = self.last_table_mut() {
            let current = table.row_offset();
            let new_offset = current.saturating_sub(amount);
            table.set_row_offset(new_offset);
        }
    }

    pub fn scroll_last_table_down(&mut self, amount: usize) {
        if let Some(table) = self.last_table_mut() {
            let total = table.total_rows();
            let display = MAX_TABLE_DISPLAY_ROWS;
            let current = table.row_offset();
            let max_offset = total.saturating_sub(display);
            let new_offset = (current + amount).min(max_offset);
            table.set_row_offset(new_offset);
        }
    }

    pub fn reset_last_table_scroll(&mut self) {
        if let Some(table) = self.last_table_mut() {
            table.set_row_offset(0);
        }
    }

    pub fn replace_with(&mut self, lines: Vec<OutputLine>) {
        self.lines.clear();
        self.scroll_offset = 0;
        for line in lines {
            self.lines.push_back(line);
        }
    }

    pub fn snapshot(&self) -> Vec<OutputLine> {
        self.lines.iter().cloned().collect()
    }

    pub fn visible_range(&self, visible_height: usize) -> (usize, Vec<&OutputLine>) {
        let total = self.lines.len();
        if total == 0 {
            return (0, Vec::new());
        }

        let mut line = 0usize;
        let mut start_idx = total;
        let mut visible_items: Vec<&OutputLine> = Vec::new();

        for (i, item) in self.lines.iter().enumerate().rev() {
            let h = item.visual_height();
            let item_end = line + h;

            if item_end > self.scroll_offset && line < self.scroll_offset + visible_height {
                visible_items.push(item);
                start_idx = i;
            }

            if line >= self.scroll_offset + visible_height {
                break;
            }

            line = item_end;
        }
        visible_items.reverse();

        let hidden: usize = self.lines
            .iter()
            .take(start_idx)
            .map(|l| l.visual_height())
            .sum();

        (hidden, visible_items)
    }
}
