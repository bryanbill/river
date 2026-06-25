use crate::tui::highlight;
use crate::tui::input::InputState;
use crate::tui::theme::Theme;

#[test]
fn input_insert_text_with_newlines() {
    let mut input = InputState::new();
    input.insert_text("hello\nworld\n");
    assert_eq!(input.text, "hello\nworld\n");
    assert_eq!(input.line_count(), 3);
}

#[test]
fn input_insert_text_strips_carriage_returns() {
    let mut input = InputState::new();
    input.insert_text("hello\r\nworld");
    assert_eq!(input.text, "hello\nworld");
}

#[test]
fn input_cursor_navigation_multiline() {
    let mut input = InputState::new();
    input.insert_text("line1\nline2\nline3");
    input.move_cursor_home();
    assert_eq!(input.cursor_line_col(), (2, 0));

    input.move_cursor_up();
    assert_eq!(input.cursor_line_col(), (1, 0));

    input.move_cursor_up();
    assert_eq!(input.cursor_line_col(), (0, 0));

    input.move_cursor_up();
    assert_eq!(input.cursor_line_col(), (0, 0));

    input.move_cursor_end();
    assert_eq!(input.cursor_line_col(), (0, 5));
}

#[test]
fn input_delete_word_before() {
    let mut input = InputState::new();
    input.insert_text("find users where");
    input.cursor_pos = input.text.len();

    input.delete_word_before();
    assert_eq!(input.text, "find users ");
    assert_eq!(input.cursor_pos, 11);

    input.delete_word_before();
    assert_eq!(input.text, "find ");
    assert_eq!(input.cursor_pos, 5);
}

#[test]
fn input_delete_word_before_multiline() {
    let mut input = InputState::new();
    input.insert_text("find *\nfrom users");
    input.cursor_pos = input.text.len();

    input.delete_word_before();
    assert_eq!(input.text, "find *\nfrom ");
    assert_eq!(input.cursor_pos, 12);
}

#[test]
fn input_submit_clears_text() {
    let mut input = InputState::new();
    input.insert_text("find *\nfrom users");
    let cmd = input.submit();
    assert_eq!(cmd, "find *\nfrom users");
    assert!(input.is_empty());
    assert_eq!(input.history.len(), 1);
    assert_eq!(input.history[0], "find *\nfrom users");
}

#[test]
fn highlight_token_kind_classification() {
    use crate::lang::lexer::Token;
    use highlight::TokenKind;

    assert_eq!(highlight::token_kind(&Token::Find), TokenKind::Keyword);
    assert_eq!(highlight::token_kind(&Token::Where), TokenKind::Keyword);
    assert_eq!(
        highlight::token_kind(&Token::StringLit("hi".into())),
        TokenKind::String
    );
    assert_eq!(highlight::token_kind(&Token::Integer(42)), TokenKind::Number);
    assert_eq!(highlight::token_kind(&Token::Float(std::f64::consts::PI)), TokenKind::Number);
    assert_eq!(highlight::token_kind(&Token::True), TokenKind::Bool);
    assert_eq!(highlight::token_kind(&Token::Null), TokenKind::Null);
    assert_eq!(highlight::token_kind(&Token::Eq), TokenKind::Operator);
    assert_eq!(highlight::token_kind(&Token::LParen), TokenKind::Punctuation);
    assert_eq!(
        highlight::token_kind(&Token::Ident("users".into())),
        TokenKind::Identifier
    );
    assert_eq!(
        highlight::token_kind(&Token::Param("name".into())),
        TokenKind::Parameter
    );
    assert_eq!(highlight::token_kind(&Token::Count), TokenKind::Function);
}

#[test]
fn highlight_line_basic() {
    let theme = Theme::default();
    let input = "find * from users";
    let tokens = highlight::tokenize(input);

    let spans = highlight::highlight_line(input, 0, &tokens, None, &theme);
    assert!(!spans.is_empty());
}

#[test]
fn highlight_line_with_cursor() {
    let theme = Theme::default();
    let input = "find * from users";
    let tokens = highlight::tokenize(input);

    let spans = highlight::highlight_line(input, 0, &tokens, Some(0), &theme);
    assert!(!spans.is_empty());
}

#[test]
fn highlight_line_multiline() {
    let theme = Theme::default();
    let input = "find *\nfrom users\nwhere age > 21";
    let tokens = highlight::tokenize(input);

    let line1_start = highlight::line_start_byte(input, 1);
    assert_eq!(line1_start, 7);

    let line2_start = highlight::line_start_byte(input, 2);
    assert_eq!(line2_start, 18);

    let lines: Vec<&str> = input.split('\n').collect();
    let spans = highlight::highlight_line(lines[1], line1_start, &tokens, None, &theme);
    assert!(!spans.is_empty());
}

#[test]
fn highlight_line_empty() {
    let theme = Theme::default();
    let input = "";
    let tokens = highlight::tokenize(input);

    let spans = highlight::highlight_line("", 0, &tokens, Some(0), &theme);
    assert!(!spans.is_empty());
}

#[test]
fn highlight_line_cursor_at_end() {
    let theme = Theme::default();
    let input = "find";
    let tokens = highlight::tokenize(input);

    let spans = highlight::highlight_line(input, 0, &tokens, Some(4), &theme);
    assert!(!spans.is_empty());
}

#[test]
fn line_start_byte_empty_lines() {
    let input = "a\n\nb";
    assert_eq!(highlight::line_start_byte(input, 0), 0);
    assert_eq!(highlight::line_start_byte(input, 1), 2);
    assert_eq!(highlight::line_start_byte(input, 2), 3);
}
