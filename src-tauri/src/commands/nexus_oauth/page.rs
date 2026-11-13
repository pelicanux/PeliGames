use std::io::Write;

pub(super) enum Outcome {
    Received,
    Cancelled,
    Invalid,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(super) fn render(outcome: &Outcome, language: &str) -> String {
    let (language, catalog) = if language == "pt-BR" || language == "pt" {
        (
            "pt-BR",
            include_str!("../../../../src/i18n/locales/pt-BR/nexusCallback.json"),
        )
    } else {
        (
            "en",
            include_str!("../../../../src/i18n/locales/en/nexusCallback.json"),
        )
    };
    let messages: std::collections::HashMap<String, String> =
        serde_json::from_str(catalog).expect("valid bundled Nexus callback translations");
    let key = match outcome {
        Outcome::Received => "received",
        Outcome::Cancelled => "cancelled",
        Outcome::Invalid => "invalid",
    };
    let text = |suffix: &str| escape(&messages[&format!("{key}_{suffix}")]);
    include_str!("callback.html")
        .replace("{{LANGUAGE}}", language)
        .replace("{{BRAND_STYLES}}", include_str!("../connection-brand.css"))
        .replace(
            "{{BRAND_MARKUP}}",
            include_str!("../connection-brand-markup.html"),
        )
        .replace(
            "{{STATUS}}",
            if matches!(outcome, Outcome::Received) {
                ""
            } else {
                "error"
            },
        )
        .replace(
            "{{ICON}}",
            if matches!(outcome, Outcome::Received) {
                "<path d=\"M5 12h14m-6-6 6 6-6 6\"/>"
            } else {
                "<path d=\"M12 5v8m0 5h.01\"/>"
            },
        )
        .replace("{{TITLE}}", &text("title"))
        .replace("{{MESSAGE}}", &text("message"))
        .replace("{{NEXT}}", &text("next"))
        .replace("{{CLOSE}}", &escape(&messages["close"]))
}

pub(super) fn write(stream: &mut impl Write, outcome: Outcome, language: &str) {
    let body = render(&outcome, language);
    let status = if matches!(outcome, Outcome::Invalid) {
        "400 Bad Request"
    } else {
        "200 OK"
    };
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\nConnection: close\r\n\r\n{body}", body.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn received_response_is_not_a_claim_of_a_connected_account() {
        for (language, title, next) in [
            ("pt-BR", "Autorização recebida", "verificar sua conta"),
            ("en", "Authorization received", "verify your account"),
        ] {
            let body = render(&Outcome::Received, language);
            assert!(body.contains(title) && body.contains(next));
            assert!(body.contains(&format!("lang=\"{language}\"")));
            assert!(!body.contains("{{") && !body.contains("<script"));
            assert!(!body.contains("<link") && !body.contains("<form"));
        }
        assert!(render(&Outcome::Received, "de").contains("lang=\"en\""));
    }

    #[test]
    fn cancelled_and_invalid_callbacks_have_distinct_guidance() {
        for language in ["pt-BR", "en"] {
            let cancelled = render(&Outcome::Cancelled, language);
            let invalid = render(&Outcome::Invalid, language);
            assert!(!cancelled.contains("{{") && !invalid.contains("{{"));
            assert_ne!(cancelled, invalid);
            assert!(cancelled.contains("status error") && invalid.contains("status error"));
        }
    }

    #[test]
    fn html_response_has_correct_byte_length_and_private_headers() {
        for outcome in [Outcome::Received, Outcome::Cancelled, Outcome::Invalid] {
            let mut bytes = Vec::new();
            let status = if matches!(outcome, Outcome::Invalid) {
                "400 Bad Request"
            } else {
                "200 OK"
            };
            write(&mut bytes, outcome, "pt-BR");
            let response = String::from_utf8(bytes).unwrap();
            let (headers, body) = response.split_once("\r\n\r\n").unwrap();
            assert!(headers.starts_with(&format!("HTTP/1.1 {status}")));
            assert!(headers.contains(&format!("Content-Length: {}", body.len())));
            assert!(headers.contains("Content-Type: text/html; charset=utf-8"));
            assert!(headers.contains("Cache-Control: no-store"));
            assert!(headers.contains("Referrer-Policy: no-referrer"));
            assert!(headers.contains("frame-ancestors 'none'"));
        }
    }
}
