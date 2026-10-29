//! Conservative redaction shared by every support report, including free text.
use serde_json::{Map, Value};

pub fn sanitize(text: &str) -> String {
    let mut private_block = false;
    text.lines()
        .map(|line| {
            let clean: String = line
                .chars()
                .filter(|c| !c.is_control() || *c == '\t')
                .collect();
            let lower = clean.to_ascii_lowercase();
            if lower.contains("-----begin") && lower.contains("private key") {
                private_block = true;
            }
            if private_block {
                if lower.contains("-----end") {
                    private_block = false;
                }
                return "[bloco privado removido]".to_string();
            }
            if [
                "password",
                "passwd",
                "senha",
                "secret",
                "token",
                "authorization",
                "api_key",
                "apikey",
                "api-key",
                "cookie",
                "bearer ",
                "nxm://",
                "signature=",
                "key=",
                "private key",
                "launchoptions",
                "launch_options",
                "username",
                "user_name",
                "hostname",
                "cmd\t",
                "command:",
                "arguments:",
                "argv=",
                "authcode",
                "auth_code",
            ]
            .iter()
            .any(|key| lower.contains(key))
            {
                return "[linha com dados privados removida]".to_string();
            }
            // Paths may contain spaces: redact to the next quote or delimiter.
            let mut result = String::new();
            let mut chars = clean.char_indices().peekable();
            while let Some((i, c)) = chars.next() {
                let rest = &clean[i..];
                let path = (c == '/'
                    && (i == 0
                        || !clean[..i]
                            .chars()
                            .next_back()
                            .is_some_and(|p| p.is_alphanumeric())))
                    || (c.is_ascii_alphabetic()
                        && (rest.as_bytes().get(1..3) == Some(b":\\")
                            || rest.as_bytes().get(1..3) == Some(b":/")))
                    || rest.starts_with("\\\\")
                    || rest
                        .get(..14)
                        .is_some_and(|s| s.eq_ignore_ascii_case("drive_c/users/"));
                let url = rest.starts_with("https://") || rest.starts_with("http://");
                if path || url {
                    result.push_str(if url {
                        "[endereço removido]"
                    } else {
                        "[caminho removido]"
                    });
                    while chars
                        .peek()
                        .is_some_and(|(_, ch)| !matches!(ch, '\'' | '"' | ';' | '>' | '<'))
                    {
                        chars.next();
                    }
                } else {
                    result.push(c);
                }
            }
            result
                .split_whitespace()
                .map(|word| {
                    let network = word
                        .split(|c: char| {
                            !(c.is_ascii_hexdigit() || matches!(c, ':' | '.' | '[' | ']'))
                        })
                        .any(|part| {
                            part.parse::<std::net::IpAddr>().is_ok()
                                || part.parse::<std::net::SocketAddr>().is_ok()
                                || (part.len() == 17 && part.split(':').count() == 6)
                        });
                    let id = word.chars().filter(|c| c.is_ascii_digit()).count() >= 17;
                    let opaque = word
                        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                        .any(|part| {
                            uuid::Uuid::parse_str(part).is_ok()
                                || (part.len() >= 32 && part.chars().all(|c| c.is_ascii_hexdigit()))
                        });
                    if word.contains('@') || network || id || opaque {
                        "[identificador removido]".to_string()
                    } else {
                        word.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Unknown fields, nested objects, source names and raw arguments are excluded.
pub fn select(value: &Value, fields: &[&str]) -> Value {
    let mut out = Map::new();
    for key in fields {
        if let Some(value) = value.get(*key) {
            if value.is_boolean() || value.is_number() || value.is_null() {
                out.insert((*key).into(), value.clone());
            } else if let Some(text) = value.as_str() {
                out.insert(
                    (*key).into(),
                    Value::String(sanitize(&text.chars().take(160).collect::<String>())),
                );
            }
        }
    }
    Value::Object(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_credentials_addresses_accounts_and_paths_including_spaces() {
        let input = "API_KEY=hidden\nAuthorization: Bearer hidden\ncookie: hidden\nSenha: hidden\nerror '/mnt/My Games/Alice/private file' errno=2\nC:\\Users\\Alice\\Videos\\clip.mkv\nhttp://user:pass@example.com/private?q=hidden\nmail alice@example.org\nip 192.168.1.3:2456 2001:db8::1\nSteam 76561198186131930\nMAC aa:bb:cc:dd:ee:ff\nSwapchain: 1920x1080 AV1 error 2";
        let output = sanitize(input);
        for secret in [
            "hidden",
            "Alice",
            "private file",
            "user:pass",
            "alice@example",
            "192.168.1.3",
            "2001:db8",
            "765611",
            "aa:bb",
        ] {
            assert!(!output.contains(secret), "{secret}: {output}");
        }
        assert!(output.contains("errno=2"));
        assert!(output.contains("1920x1080 AV1 error 2"));
        assert_eq!(sanitize(&output), output);
    }
    #[test]
    fn removes_multiline_keys_wine_users_and_bracketed_ipv6() {
        let output = sanitize("-----BEGIN PRIVATE KEY-----\nbase64SensitiveMaterial\n-----END PRIVATE KEY-----\nfile drive_c/users/Alice/private.dll\n[2001:db8::1]:2456\n0123456789abcdef0123456789abcdef\nAFMF: 60 FPS");
        for secret in ["Sensitive", "Alice", "2001:", "abcdef"] {
            assert!(!output.contains(secret), "{output}");
        }
        assert!(output.contains("AFMF: 60 FPS"));
        assert_eq!(sanitize(&output), output);
    }
    #[test]
    fn handles_addresses_inside_assignments_and_compact_json() {
        let result=sanitize("server=192.168.0.1:2456\n{\"ip\":\"2001:db8::1\"}\nMAC=aa:bb:cc:dd:ee:ff\nid=abcdefabcdefabcdefabcdefabcdefab\nFPS=144");
        for private in ["192.168", "2001:", "aa:bb", "abcdef"] {
            assert!(!result.contains(private), "{result}");
        }
        assert!(result.contains("FPS=144"));
        assert_eq!(sanitize(&result), result);
    }
    #[test]
    fn allowlist_drops_unrelated_metadata_and_nested_secrets() {
        let output = select(
            &serde_json::json!({"fps":60,"codec":"av1","input_source":"Alice mic", "unknown":{"secret":"hidden"}}),
            &["fps", "codec"],
        );
        assert_eq!(output, serde_json::json!({"fps":60,"codec":"av1"}));
    }
}
