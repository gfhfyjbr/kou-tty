use kou_tty::terminal::Grid;
use kou_tty::terminal::TerminalEvent;
use kou_tty::terminal::ansi::AnsiHandler;

fn feed(grid: &mut Grid, events: &mut Vec<TerminalEvent>, bytes: &[u8]) {
    let mut parser = vte::Parser::new();
    let mut handler = AnsiHandler::new(grid, events);
    parser.advance(&mut handler, bytes);
}

#[test]
fn plain_text_is_written_into_grid() {
    let mut grid = Grid::new(2, 16);
    let mut events: Vec<TerminalEvent> = Vec::new();
    feed(&mut grid, &mut events, b"hello world");
    assert_eq!(grid.row_text(0), "hello world");
}

#[test]
fn newline_and_carriage_return_move_cursor() {
    let mut grid = Grid::new(3, 16);
    let mut events: Vec<TerminalEvent> = Vec::new();
    feed(&mut grid, &mut events, b"one\r\ntwo\r\nthree");
    assert_eq!(grid.row_text(0), "one");
    assert_eq!(grid.row_text(1), "two");
    assert_eq!(grid.row_text(2), "three");
}

#[test]
fn csi_cursor_position_then_print() {
    let mut grid = Grid::new(5, 16);
    let mut events: Vec<TerminalEvent> = Vec::new();
    feed(&mut grid, &mut events, b"\x1b[3;5HX");
    assert_eq!(grid.cursor_row, 2);
    assert_eq!(grid.cursor_col, 5);
    let row = grid.row_text(2);
    assert!(row.ends_with('X'), "expected trailing X, got {row:?}");
}

#[test]
fn csi_erase_line_clears_after_cursor() {
    let mut grid = Grid::new(2, 8);
    let mut events: Vec<TerminalEvent> = Vec::new();
    feed(&mut grid, &mut events, b"abcdef");
    feed(&mut grid, &mut events, b"\x1b[3D"); // cursor back 3 → col 3
    feed(&mut grid, &mut events, b"\x1b[K"); // erase to EOL
    assert_eq!(grid.row_text(0), "abc");
}

#[test]
fn sgr_reset_clears_attributes() {
    let mut grid = Grid::new(1, 4);
    let mut events: Vec<TerminalEvent> = Vec::new();
    feed(&mut grid, &mut events, b"\x1b[1;31ma\x1b[0mb");
    assert!(grid.cells[0][0].attrs.bold);
    assert!(!grid.cells[0][1].attrs.bold);
}

#[test]
fn bell_emits_event() {
    let mut grid = Grid::new(1, 4);
    let mut events: Vec<TerminalEvent> = Vec::new();
    feed(&mut grid, &mut events, b"\x07");
    assert!(events.iter().any(|e| matches!(e, TerminalEvent::Bell)));
}
