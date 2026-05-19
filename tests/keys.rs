use kou_tty::terminal::keys::{encode_key, encode_mouse_sgr};

#[test]
fn named_keys_encode_to_known_byte_sequences() {
    assert_eq!(encode_key("Enter").as_deref(), Some(b"\r".as_ref()));
    assert_eq!(encode_key("Tab").as_deref(), Some(b"\t".as_ref()));
    assert_eq!(encode_key("Escape").as_deref(), Some(b"\x1b".as_ref()));
    assert_eq!(encode_key("Backspace").as_deref(), Some(b"\x7f".as_ref()));
    assert_eq!(encode_key("Up").as_deref(), Some(b"\x1b[A".as_ref()));
    assert_eq!(encode_key("Down").as_deref(), Some(b"\x1b[B".as_ref()));
    assert_eq!(encode_key("Right").as_deref(), Some(b"\x1b[C".as_ref()));
    assert_eq!(encode_key("Left").as_deref(), Some(b"\x1b[D".as_ref()));
    assert_eq!(encode_key("F1").as_deref(), Some(b"\x1bOP".as_ref()));
    assert_eq!(encode_key("F5").as_deref(), Some(b"\x1b[15~".as_ref()));
}

#[test]
fn key_names_are_case_insensitive() {
    assert_eq!(encode_key("enter"), encode_key("Enter"));
    assert_eq!(encode_key("ESCAPE"), encode_key("Escape"));
    assert_eq!(encode_key("ArrowUp"), encode_key("Up"));
}

#[test]
fn ctrl_modifier_encodes_to_control_byte() {
    assert_eq!(encode_key("ctrl+a").as_deref(), Some(b"\x01".as_ref()));
    assert_eq!(encode_key("ctrl+c").as_deref(), Some(b"\x03".as_ref()));
    assert_eq!(encode_key("Ctrl+D").as_deref(), Some(b"\x04".as_ref()));
}

#[test]
fn alt_prefix_emits_escape_then_text() {
    let bytes = encode_key("alt+f").expect("alt key");
    assert_eq!(bytes, b"\x1bf".to_vec());
}

#[test]
fn unknown_key_returns_none() {
    assert!(encode_key("nope").is_none());
    assert!(encode_key("ctrl+ctrl").is_none());
}

#[test]
fn mouse_sgr_encoding_uses_one_indexed_coords() {
    let bytes = encode_mouse_sgr(0, 4, 9, true);
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), "\x1b[<0;5;10M");
    let bytes = encode_mouse_sgr(2, 0, 0, false);
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), "\x1b[<2;1;1m");
}
