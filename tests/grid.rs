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
