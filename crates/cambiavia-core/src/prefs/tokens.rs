#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub name: String,
    pub value: String,
}

fn parse_piece(piece: &str) -> Option<Token> {
    let piece = piece.trim();
    if piece.is_empty() {
        return None;
    }
    match piece.split_once('=') {
        Some((name, value)) => Some(Token {
            name: name.trim().to_string(),
            value: value.trim().to_string(),
        }),
        None => Some(Token {
            name: piece.to_string(),
            value: String::new(),
        }),
    }
}

pub fn parse(text: &str) -> Vec<Token> {
    text.split(';').filter_map(parse_piece).collect()
}

pub fn format(tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        out.push_str(&token.name);
        out.push('=');
        out.push_str(&token.value);
        out.push(';');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_peak_before_value() {
        assert_eq!(
            parse("AppStatus=0;AutoHDREnable=2097;"),
            vec![
                Token {
                    name: "AppStatus".to_string(),
                    value: "0".to_string(),
                },
                Token {
                    name: "AutoHDREnable".to_string(),
                    value: "2097".to_string(),
                },
            ]
        );
    }

    #[test]
    fn trims_trailing_space() {
        assert_eq!(
            parse("GpuPreference=1; "),
            vec![Token {
                name: "GpuPreference".to_string(),
                value: "1".to_string(),
            }]
        );
    }

    #[test]
    fn round_trip_keeps_bytes() {
        for value in [
            "AppStatus=0;AutoHDREnable=2097;",
            "AppStatus=0;AutoHDREnable=2097;SpecificAdapter=1002&747E&2427148C;GpuPreference=1073741824;",
            "SwapEffectUpgradeEnable=1;AutoHDREnable=1;",
        ] {
            assert_eq!(format(&parse(value)), value);
        }
    }

    #[test]
    fn empty_list_formats_to_empty_string() {
        assert_eq!(format(&[]), "");
        assert!(parse("").is_empty());
        assert!(parse(";;;").is_empty());
        assert!(parse("   ").is_empty());
    }

    #[test]
    fn unknown_tokens_keep_order() {
        let tokens = parse("AppStatus=0;FutureThing=abc;AutoHDREnable=2097;");
        let names: Vec<&str> = tokens.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["AppStatus", "FutureThing", "AutoHDREnable"]);
    }

    #[test]
    fn name_only_piece_is_kept() {
        let tokens = parse("Foo;Bar=1;");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].name, "Foo");
    }

    #[test]
    fn malformed_input_never_panics() {
        for bad in [
            "===;",
            "=;",
            ";=;",
            "A=B=C;",
            "NoEquals",
            ";;;===;;;",
            "GpuPreference=1;",
        ] {
            let tokens = parse(bad);
            let _ = format(&tokens);
        }
    }
}
