//! Secret scrubbing for everything ingested from transcripts, AGY artifacts and git diffs.
use regex::Regex;
use std::sync::LazyLock;

const MASK: &str = "[REDACTED]";

/// Applied in order: bearer headers must go before the `.env`-style rule, which would
/// otherwise take `Bearer` as the value of `Authorization:` and leave the token behind.
static RULES: LazyLock<[(Regex, String); 6]> = LazyLock::new(|| {
    let rule = |re: &str, to: &str| (Regex::new(re).expect("static regex"), to.to_string());
    [
        rule(
            r"-----BEGIN [A-Z ]*PRIVATE KEY(?: BLOCK)?-----[\s\S]*?(-----END [A-Z ]*PRIVATE KEY(?: BLOCK)?-----|$)",
            MASK,
        ),
        rule(r"(?i)\bbearer\s+[\w.~+/=-]{8,}", &format!("Bearer {MASK}")),
        rule(
            r"(?i)\bbasic\s+[A-Za-z0-9+/=]{8,}",
            &format!("Basic {MASK}"),
        ),
        rule(r"(\w+://[^\s:/@]+:)[^\s@/]+@", &format!("${{1}}{MASK}@")),
        rule(
            r"\b(sk-[\w-]{8,}|gh[pousr]_\w{20,}|github_pat_\w{20,}|xox[abprs]-[\w-]{10,}|AKIA[0-9A-Z]{16}|AIza[\w-]{30,})",
            MASK,
        ),
        rule(
            r#"(?i)\b(\w*(?:key|token|secret|passw(?:or)?d|credential)\w*)(["']?\s*[=:]\s*)("[^"]*"|'[^']*'|[^\s"',}]+)"#,
            &format!("$1$2{MASK}"),
        ),
    ]
});

/// Masks API keys, tokens, bearer headers, private keys and `.env`-style secret values.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for (re, to) in RULES.iter() {
        out = re.replace_all(&out, to.as_str()).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_each_secret_shape() {
        for (input, leaked) in [
            (
                "curl -H 'Authorization: Bearer abcDEF123456.xyz'",
                "abcDEF123456",
            ),
            ("key sk-ant-api03-AbCdEf123456", "AbCdEf123456"),
            ("ghp_0123456789abcdefghijABCDEFGHIJ", "0123456789"),
            ("AWS AKIAIOSFODNN7EXAMPLE here", "AKIAIOSFODNN7EXAMPLE"),
            ("OPENAI_API_KEY=hunter2hunter2", "hunter2"),
            ("DB_PASSWORD: \"p w d\"", "p w d"),
            (r#"{"api_key": "abc123xyz", "n": 1}"#, "abc123xyz"),
            ("Authorization: Basic dXNlcjpwYXNzd29yZA==", "dXNlcjpwYXNz"),
            ("postgres://admin:s3cretpw@db:5432/x", "s3cretpw"),
            (
                "-----BEGIN PGP PRIVATE KEY BLOCK-----\nlQOYBF\n-----END PGP PRIVATE KEY BLOCK-----",
                "lQOYBF",
            ),
            ("export GITHUB_TOKEN='abc def'", "abc def"),
            (
                "-----BEGIN RSA PRIVATE KEY-----\nMIIBOgIBAAJB\n-----END RSA PRIVATE KEY-----",
                "MIIBOgIBAAJB",
            ),
        ] {
            let out = redact(input);
            assert!(out.contains(MASK), "{input} -> {out}");
            assert!(!out.contains(leaked), "{input} -> {out}");
        }
    }

    #[test]
    fn leaves_plain_text_alone() {
        let s = "we store the cursor key in sqlite; keyboard and monkey stay";
        assert_eq!(redact(s), s);
    }
}
