pub fn encode_key(name: &str) -> Option<Vec<u8>> {
    let lower = name.to_lowercase();
    let bytes: &[u8] = match lower.as_str() {
        "enter" | "return" | "ret" => b"\r",
        "tab" => b"\t",
        "backspace" | "bs" => b"\x7f",
        "escape" | "esc" => b"\x1b",
        "space" => b" ",
        "up" | "arrowup" => b"\x1b[A",
        "down" | "arrowdown" => b"\x1b[B",
        "right" | "arrowright" => b"\x1b[C",
        "left" | "arrowleft" => b"\x1b[D",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "pageup" | "pgup" => b"\x1b[5~",
        "pagedown" | "pgdn" => b"\x1b[6~",
        "insert" | "ins" => b"\x1b[2~",
        "delete" | "del" => b"\x1b[3~",
        "f1" => b"\x1bOP",
        "f2" => b"\x1bOQ",
        "f3" => b"\x1bOR",
        "f4" => b"\x1bOS",
        "f5" => b"\x1b[15~",
        "f6" => b"\x1b[17~",
        "f7" => b"\x1b[18~",
        "f8" => b"\x1b[19~",
        "f9" => b"\x1b[20~",
        "f10" => b"\x1b[21~",
        "f11" => b"\x1b[23~",
        "f12" => b"\x1b[24~",
        _ => return encode_modified_key(&lower),
    };
    Some(bytes.to_vec())
}

fn encode_modified_key(name: &str) -> Option<Vec<u8>> {
    if let Some(rest) = name.strip_prefix("ctrl+") {
        if rest.len() == 1 {
            let c = rest.chars().next()?;
            if c.is_ascii_alphabetic() {
                let low = c.to_ascii_lowercase() as u8;
                return Some(vec![low - b'a' + 1]);
            }
        }
        return None;
    }
    if let Some(rest) = name.strip_prefix("alt+") {
        let mut bytes = vec![0x1b];
        bytes.extend_from_slice(rest.as_bytes());
        return Some(bytes);
    }
    None
}

pub fn encode_mouse_sgr(button: u16, col: u16, row: u16, pressed: bool) -> Vec<u8> {
    let suffix = if pressed { 'M' } else { 'm' };
    format!("\x1b[<{};{};{}{}", button, col + 1, row + 1, suffix).into_bytes()
}
