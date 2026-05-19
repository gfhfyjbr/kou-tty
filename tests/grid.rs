use kou_tty::terminal::Grid;

#[test]
fn put_char_advances_cursor_and_marks_dirty() {
    let mut grid = Grid::new(24, 80);
    let _ = grid.take_dirty();

    grid.put_char('h');
    grid.put_char('i');

    assert_eq!(grid.cursor_row, 0);
    assert_eq!(grid.cursor_col, 2);
    let dirty = grid.take_dirty();
    assert_eq!(dirty, vec![0]);
    assert_eq!(grid.row_text(0), "hi");
}

#[test]
fn line_feed_below_last_row_scrolls_into_scrollback() {
    let mut grid = Grid::new(2, 4);

    for _ in 0..2 {
        for ch in ['a', 'b', 'c'] {
            grid.put_char(ch);
        }
        grid.carriage_return();
        grid.line_feed();
    }

    // The first row should have been pushed into scrollback by now.
    assert!(!grid.scrollback.is_empty());
    assert_eq!(grid.cursor_row, 1);
    assert_eq!(grid.cursor_col, 0);
}

#[test]
fn carriage_return_and_backspace() {
    let mut grid = Grid::new(2, 8);
    for ch in "hello".chars() {
        grid.put_char(ch);
    }
    grid.carriage_return();
    assert_eq!(grid.cursor_col, 0);
    grid.put_char('w');
    assert_eq!(grid.row_text(0), "wello");

    grid.backspace();
    assert_eq!(grid.cursor_col, 0);
    grid.cursor_forward(2);
    grid.backspace();
    assert_eq!(grid.cursor_col, 1);
}

#[test]
fn erase_in_display_modes() {
    let mut grid = Grid::new(3, 4);
    for r in 0..3 {
        for c in 0..4 {
            grid.cursor_move(r, c);
            grid.put_char('x');
        }
    }
    assert_eq!(grid.row_text(0), "xxxx");

    grid.cursor_move(1, 2);
    grid.erase_in_display(0); // from cursor to end
    assert_eq!(grid.row_text(0), "xxxx");
    assert_eq!(grid.row_text(1), "xx");
    assert_eq!(grid.row_text(2), "");

    grid.erase_in_display(2); // whole screen
    assert_eq!(grid.row_text(0), "");
    assert_eq!(grid.row_text(2), "");
}

#[test]
fn resize_preserves_top_left_content() {
    let mut grid = Grid::new(5, 10);
    for ch in "hello".chars() {
        grid.put_char(ch);
    }
    grid.resize(3, 6);
    assert_eq!(grid.rows, 3);
    assert_eq!(grid.cols, 6);
    assert_eq!(grid.row_text(0), "hello");
}

#[test]
fn plain_text_contains_all_rows_separated_by_newline() {
    let mut grid = Grid::new(3, 4);
    for ch in "ab".chars() {
        grid.put_char(ch);
    }
    grid.carriage_return();
    grid.line_feed();
    for ch in "cd".chars() {
        grid.put_char(ch);
    }
    let text = grid.plain_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines, ["ab", "cd", ""]);
}

#[test]
fn row_text_ansi_emits_sgr_for_styled_cells() {
    use kou_tty::terminal::{CellAttrs, Color};

    let mut grid = Grid::new(1, 8);
    grid.current_fg = Color::Indexed(1);
    grid.put_char('R');
    grid.current_fg = Color::Indexed(2);
    grid.current_attrs = CellAttrs {
        bold: true,
        ..Default::default()
    };
    grid.put_char('G');
    grid.put_char('G');

    let ansi = grid.row_text_ansi(0);
    assert!(ansi.contains("\x1b[31m"), "missing red SGR in {ansi:?}");
    assert!(ansi.contains("32"), "missing green SGR in {ansi:?}");
    assert!(ansi.contains("1;"), "missing bold SGR in {ansi:?}");
    assert!(ansi.ends_with("\x1b[0m"), "missing reset at end: {ansi:?}");
    assert!(ansi.contains('R'));
    assert!(ansi.contains('G'));
}

#[test]
fn row_text_ansi_skips_redundant_sgr_for_identical_runs() {
    use kou_tty::terminal::Color;

    let mut grid = Grid::new(1, 4);
    grid.current_fg = Color::Indexed(1);
    for ch in "RRRR".chars() {
        grid.put_char(ch);
    }
    let ansi = grid.row_text_ansi(0);
    let sgr_count = ansi.matches("\x1b[0m").count();
    // One opening reset + one closing reset is expected.
    assert!(sgr_count >= 1 && sgr_count <= 2, "got {sgr_count}");
    assert!(ansi.ends_with("\x1b[0m"));
}

#[test]
fn row_text_ansi_returns_empty_for_blank_row() {
    let grid = Grid::new(2, 8);
    assert_eq!(grid.row_text_ansi(0), "");
}
