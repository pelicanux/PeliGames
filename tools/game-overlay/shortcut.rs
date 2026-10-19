//! Shared physical-key contract for the recorder, launch helper and game layer.
//! Values are Linux evdev codes, independent of the desktop/compositor.
pub fn parse(text: &str) -> Result<(u32, u8), String> {
    if text.len() > 100 {
        return Err("Atalho excede o limite.".into());
    }
    let mut key = None;
    let mut modifiers = 0;
    for part in text.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "control_l" | "control_r" => modifiers |= 1,
            "shift_l" | "shift_r" => modifiers |= 2,
            "alt_l" | "alt_r" => modifiers |= 4,
            "super_l" | "super_r" => modifiers |= 8,
            other => {
                if key.is_some() {
                    return Err("Escolha uma tecla com modificadores opcionais.".into());
                }
                let code = evdev(other).ok_or("Tecla não suportada pelo overlay.")?;
                if code == 1 {
                    return Err(
                        "Esc é reservado ao jogo. Escolha outra tecla para o overlay.".into(),
                    );
                }
                key = Some(code);
            }
        }
    }
    Ok((key.ok_or("Escolha uma tecla para o overlay.")?, modifiers))
}

fn evdev(name: &str) -> Option<u32> {
    let normalized = name.to_ascii_lowercase();
    if let Some(n) = normalized
        .strip_prefix('f')
        .and_then(|n| n.parse::<u32>().ok())
    {
        return match n {
            1..=10 => Some(58 + n),
            11 => Some(87),
            12 => Some(88),
            13..=24 => Some(170 + n),
            _ => None,
        };
    }
    let names = [
        ("escape", 1),
        ("return", 28),
        ("space", 57),
        ("tab", 15),
        ("backspace", 14),
        ("insert", 110),
        ("delete", 111),
        ("home", 102),
        ("end", 107),
        ("page_up", 104),
        ("page_down", 109),
        ("left", 105),
        ("right", 106),
        ("up", 103),
        ("down", 108),
        ("1", 2),
        ("2", 3),
        ("3", 4),
        ("4", 5),
        ("5", 6),
        ("6", 7),
        ("7", 8),
        ("8", 9),
        ("9", 10),
        ("0", 11),
        ("q", 16),
        ("w", 17),
        ("e", 18),
        ("r", 19),
        ("t", 20),
        ("y", 21),
        ("u", 22),
        ("i", 23),
        ("o", 24),
        ("p", 25),
        ("a", 30),
        ("s", 31),
        ("d", 32),
        ("f", 33),
        ("g", 34),
        ("h", 35),
        ("j", 36),
        ("k", 37),
        ("l", 38),
        ("z", 44),
        ("x", 45),
        ("c", 46),
        ("v", 47),
        ("b", 48),
        ("n", 49),
        ("m", 50),
        ("minus", 12),
        ("equal", 13),
        ("bracketleft", 26),
        ("bracketright", 27),
        ("semicolon", 39),
        ("apostrophe", 40),
        ("grave", 41),
        ("backslash", 43),
        ("comma", 51),
        ("period", 52),
        ("slash", 53),
        ("pause", 119),
        ("caps_lock", 58),
        ("scroll_lock", 70),
        ("print", 99),
        ("kp_0", 82),
        ("kp_1", 79),
        ("kp_2", 80),
        ("kp_3", 81),
        ("kp_4", 75),
        ("kp_5", 76),
        ("kp_6", 77),
        ("kp_7", 71),
        ("kp_8", 72),
        ("kp_9", 73),
        ("kp_enter", 96),
        ("kp_add", 78),
        ("kp_subtract", 74),
        ("kp_multiply", 55),
        ("kp_divide", 98),
        ("kp_decimal", 83),
    ];
    names
        .into_iter()
        .find_map(|(n, k)| (n == normalized).then_some(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_and_custom_physical_keys() {
        assert_eq!(parse("F8").unwrap(), (66, 0));
        assert_eq!(parse("shift_l+f8").unwrap(), (66, 2));
        assert_eq!(parse("Control_R+Shift_L+F12").unwrap(), (88, 3));
        assert_eq!(parse("KP_Add").unwrap(), (78, 0));
        assert!(parse("Escape").is_err());
        assert!(parse("F8+F9").is_err());
        assert!(parse("Shift_L").is_err());
    }
}
