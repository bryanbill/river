use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::lang::lexer::{lex, Spanned, Token};
use crate::tui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Function,
    String,
    Number,
    Operator,
    Parameter,
    Identifier,
    Punctuation,
    Bool,
    Null,
}

pub fn token_kind(token: &Token) -> TokenKind {
    match token {
        Token::StringLit(_) => TokenKind::String,
        Token::Integer(_) | Token::Float(_) | Token::Interval(_, _) => TokenKind::Number,
        Token::Param(_) => TokenKind::Parameter,
        Token::True | Token::False => TokenKind::Bool,
        Token::Null => TokenKind::Null,

        Token::Find | Token::With | Token::Recursive | Token::As | Token::Where
        | Token::From | Token::Join | Token::Left | Token::Right | Token::Full
        | Token::Cross | Token::Inner | Token::On | Token::Group | Token::By
        | Token::Having | Token::Order | Token::Asc | Token::Desc | Token::Nulls
        | Token::First | Token::Last | Token::Limit | Token::Offset | Token::Distinct
        | Token::Case | Token::When | Token::Then | Token::Else | Token::End
        | Token::Union | Token::All | Token::Intersect | Token::Except
        | Token::Exists | Token::In | Token::Not | Token::Between
        | Token::And | Token::Or | Token::Like | Token::ILike | Token::Is
        | Token::Create | Token::Table | Token::Update | Token::Set | Token::Remove
        | Token::Insert | Token::If | Token::Conflict | Token::Ignore | Token::Replace
        | Token::Primary | Token::Key | Token::Default_
        | Token::Explain | Token::Describe | Token::Show | Token::Tables
        | Token::Over | Token::Partition | Token::Window
        | Token::Alter | Token::Add | Token::Column | Token::Drop | Token::Rename | Token::To
        | Token::Type | Token::Cascade | Token::Restrict | Token::Database => TokenKind::Keyword,

        Token::Coalesce | Token::Nullif | Token::Ifnull | Token::Cast | Token::Now
        | Token::Count | Token::Sum | Token::Avg | Token::Min | Token::Max
        | Token::CountDistinct | Token::Any | Token::Some => TokenKind::Function,

        Token::Eq | Token::Neq | Token::Lt | Token::Gt | Token::Lte | Token::Gte
        | Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent
        | Token::Concat | Token::CastOp | Token::Arrow | Token::At => TokenKind::Operator,

        Token::LParen | Token::RParen | Token::LBracket | Token::RBracket
        | Token::LBrace | Token::RBrace | Token::Comma | Token::Dot
        | Token::Colon | Token::Semicolon => TokenKind::Punctuation,

        Token::Ident(_) => TokenKind::Identifier,
    }
}

pub fn style_for_kind(kind: TokenKind, theme: &Theme) -> Style {
    match kind {
        TokenKind::Keyword => {
            Style::default().fg(theme.syntax_keyword).add_modifier(Modifier::BOLD)
        }
        TokenKind::Function => Style::default().fg(theme.syntax_function),
        TokenKind::String => Style::default().fg(theme.syntax_string),
        TokenKind::Number => Style::default().fg(theme.syntax_number),
        TokenKind::Operator => Style::default().fg(theme.syntax_operator),
        TokenKind::Parameter => Style::default().fg(theme.syntax_parameter),
        TokenKind::Identifier => Style::default().fg(theme.syntax_identifier),
        TokenKind::Punctuation => Style::default().fg(theme.syntax_punctuation),
        TokenKind::Bool => Style::default().fg(theme.syntax_bool),
        TokenKind::Null => Style::default().fg(theme.syntax_null),
    }
}

/// A segment of highlighted text within a line.
struct Segment<'a> {
    text: &'a str,
    style: Style,
}

/// Tokenize the full input and produce highlighted spans for a single line.
///
/// `line` is the text of the line (without the trailing `\n`).
/// `line_start_byte` is the byte offset of this line within the full input.
/// `tokens` is the result of [`lex`] on the full input.
fn highlight_line_segments<'a>(
    line: &'a str,
    line_start_byte: usize,
    tokens: &'a [Spanned<Token>],
    theme: &'a Theme,
) -> Vec<Segment<'a>> {
    let line_end_byte = line_start_byte + line.len();
    let mut segments: Vec<Segment<'a>> = Vec::new();
    let mut pos = line_start_byte;

    for token in tokens {
        if token.span.end <= line_start_byte {
            continue;
        }
        if token.span.start >= line_end_byte {
            break;
        }

        if token.span.start > pos {
            let gap_start = pos.max(line_start_byte);
            let gap_end = token.span.start.min(line_end_byte);
            if gap_end > gap_start {
                let gap_text = &line[gap_start - line_start_byte..gap_end - line_start_byte];
                segments.push(Segment {
                    text: gap_text,
                    style: Style::default().fg(theme.input_text),
                });
            }
        }

        let tok_start = token.span.start.max(line_start_byte);
        let tok_end = token.span.end.min(line_end_byte);
        let tok_text = &line[tok_start - line_start_byte..tok_end - line_start_byte];
        let kind = token_kind(&token.token);
        segments.push(Segment {
            text: tok_text,
            style: style_for_kind(kind, theme),
        });

        pos = token.span.end;
    }

    if pos < line_end_byte {
        let rest = &line[pos - line_start_byte..];
        if !rest.is_empty() {
            segments.push(Segment {
                text: rest,
                style: Style::default().fg(theme.input_text),
            });
        }
    }

    segments
}

/// Produce highlighted spans for a line, with the cursor overlay applied.
///
/// `cursor_byte` is the byte offset of the cursor within `line` (not the full input).
/// If `cursor_byte` is `None`, no cursor is rendered (non-cursor lines).
pub fn highlight_line<'a>(
    line: &'a str,
    line_start_byte: usize,
    tokens: &'a [Spanned<Token>],
    cursor_byte: Option<usize>,
    theme: &'a Theme,
) -> Vec<Span<'a>> {
    let segments = highlight_line_segments(line, line_start_byte, tokens, theme);

    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut current_byte = 0usize;

    let cursor = match cursor_byte {
        Some(c) => c,
        None => {
            for seg in segments {
                spans.push(Span::styled(seg.text, seg.style));
            }
            if spans.is_empty() {
                spans.push(Span::raw(""));
            }
            return spans;
        }
    };

    for seg in segments {
        let seg_len = seg.text.len();
        let seg_end = current_byte + seg_len;

        if cursor < current_byte || cursor > seg_end {
            spans.push(Span::styled(seg.text, seg.style));
            current_byte = seg_end;
            continue;
        }

        if cursor == current_byte {
            if seg.text.is_empty() {
                spans.push(Span::styled(
                    " ",
                    theme.input_cursor_overlay(seg.style),
                ));
            } else {
                let char_len = seg.text.chars().next().map_or(0, |c| c.len_utf8());
                spans.push(Span::styled(
                    &seg.text[..char_len],
                    theme.input_cursor_overlay(seg.style),
                ));
                if seg.text.len() > char_len {
                    spans.push(Span::styled(&seg.text[char_len..], seg.style));
                }
            }
        } else if cursor < seg_end {
            let offset = cursor - current_byte;
            let char_len = seg.text[offset..].chars().next().map_or(0, |c| c.len_utf8());
            spans.push(Span::styled(&seg.text[..offset], seg.style));
            spans.push(Span::styled(
                &seg.text[offset..offset + char_len],
                theme.input_cursor_overlay(seg.style),
            ));
            if seg.text.len() > offset + char_len {
                spans.push(Span::styled(&seg.text[offset + char_len..], seg.style));
            }
        } else {
            spans.push(Span::styled(seg.text, seg.style));
        }

        current_byte = seg_end;
    }

    if cursor >= current_byte {
        spans.push(Span::styled(" ", theme.input_cursor_style()));
    }

    spans
}

/// Tokenize input for highlighting. Returns the spanned tokens.
pub fn tokenize(input: &str) -> Vec<Spanned<Token>> {
    lex(input)
}

/// Compute the byte offset of a given line within the full text.
pub fn line_start_byte(input: &str, target_line: usize) -> usize {
    if target_line == 0 {
        return 0;
    }
    let mut line = 0;
    for (i, c) in input.char_indices() {
        if c == '\n' {
            line += 1;
            if line == target_line {
                return i + 1;
            }
        }
    }
    input.len()
}
