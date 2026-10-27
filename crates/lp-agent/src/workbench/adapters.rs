//! Real `Web` and `Pdf` adapters for the workbench, bounded by timeouts and size caps.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use scraper::{Html, Selector};

use super::jobs::{Hit, Pdf, Web};

const SEARCH_URL: &str = "https://html.duckduckgo.com/html/";
const WEB_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_BODY_BYTES: usize = 2 << 20;
const MAX_HITS: usize = 20;
const PDF_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PDF_BYTES: u64 = 8 << 20;

/// Search through DuckDuckGo's HTML endpoint and fetch pages over http(s).
pub struct HttpWeb {
    client: reqwest::Client,
    search_url: String,
}

impl Default for HttpWeb {
    fn default() -> Self {
        Self::with_search_url(SEARCH_URL)
    }
}

impl HttpWeb {
    pub fn with_search_url(url: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(WEB_TIMEOUT)
            .user_agent("learnpuppy")
            .build()
            .unwrap_or_default();
        Self {
            client,
            search_url: url.into(),
        }
    }

    /// GET `req`, reading at most `MAX_BODY_BYTES` of a successful body.
    async fn body(&self, req: reqwest::RequestBuilder) -> Result<String, String> {
        let mut resp = req.send().await.map_err(|e| e.without_url().to_string())?;
        if !resp.status().is_success() {
            return Err(format!("http status {}", resp.status()));
        }
        let mut buf = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| e.without_url().to_string())?
        {
            buf.extend_from_slice(&chunk);
            if buf.len() >= MAX_BODY_BYTES {
                buf.truncate(MAX_BODY_BYTES);
                break;
            }
        }
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }
}

#[async_trait]
impl Web for HttpWeb {
    async fn search(&self, query: &str) -> Result<Vec<Hit>, String> {
        let html = self
            .body(self.client.get(&self.search_url).query(&[("q", query)]))
            .await?;
        Ok(parse_hits(&html))
    }

    async fn fetch(&self, url: &str) -> Result<String, String> {
        let url = reqwest::Url::parse(url).map_err(|e| format!("bad url: {e}"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("only http(s) urls can be fetched".into());
        }
        self.body(self.client.get(url)).await
    }
}

fn parse_hits(html: &str) -> Vec<Hit> {
    let (Ok(item), Ok(link), Ok(snip)) = (
        Selector::parse(".result"),
        Selector::parse("a.result__a"),
        Selector::parse(".result__snippet"),
    ) else {
        return Vec::new();
    };
    let doc = Html::parse_document(html);
    let text = |e: scraper::ElementRef| e.text().collect::<Vec<_>>().join(" ").trim().to_owned();
    doc.select(&item)
        .filter_map(|r| {
            let a = r.select(&link).next()?;
            Some(Hit {
                title: text(a),
                url: target(a.value().attr("href")?),
                snippet: r.select(&snip).next().map(text).unwrap_or_default(),
            })
        })
        .take(MAX_HITS)
        .collect()
}

/// DuckDuckGo wraps results as `//duckduckgo.com/l/?uddg=<url>`; unwrap them.
fn target(href: &str) -> String {
    let abs = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href.to_owned()
    };
    reqwest::Url::parse(&abs)
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "uddg")
                .map(|(_, v)| v.into_owned())
        })
        .unwrap_or(abs)
}

/// Extracts page text with the `pdftotext` binary (poppler), one string per page.
pub struct PdftotextPdf;

impl Pdf for PdftotextPdf {
    fn pages(&self, path: &str) -> Result<Vec<String>, String> {
        if !Path::new(path).is_file() {
            return Err(format!("{path} is not a file"));
        }
        // A leading `-` must not be read as an option.
        let arg = if path.starts_with('-') {
            format!("./{path}")
        } else {
            path.to_owned()
        };
        let mut child = Command::new("pdftotext")
            .args([&arg, "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("pdftotext is required to read PDFs: {e}"))?;
        let mut out = child.stdout.take().ok_or("pdftotext gave no output")?;
        // Reader thread so a full pipe never blocks the child while we watch the clock.
        let reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = (&mut out).take(MAX_PDF_BYTES).read_to_end(&mut buf);
            // Keep draining so the child can exit instead of blocking on a full pipe.
            let _ = std::io::copy(&mut out, &mut std::io::sink());
            buf
        });
        let start = Instant::now();
        let status = loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(s) => break s,
                None if start.elapsed() > PDF_TIMEOUT => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("pdftotext timed out after {PDF_TIMEOUT:?}"));
                }
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        };
        let buf = reader.join().map_err(|_| "pdftotext reader panicked")?;
        if !status.success() {
            return Err(format!("pdftotext failed on {path}"));
        }
        let mut pages: Vec<String> = String::from_utf8_lossy(&buf)
            .split('\x0c')
            .map(str::to_owned)
            .collect();
        if pages.last().is_some_and(|p| p.trim().is_empty()) {
            pages.pop();
        }
        Ok(pages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// One-shot-per-connection HTTP server: `route(path) -> (status, body)`.
    async fn serve(route: fn(&str) -> (u16, String)) -> String {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = l.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let mut buf = vec![0; 4096];
                    let n = s.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let path = req.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    let (code, body) = route(&path);
                    let head = format!(
                        "HTTP/1.1 {code} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = s.write_all(head.as_bytes()).await;
                    let _ = s.write_all(body.as_bytes()).await;
                });
            }
        });
        format!("http://{addr}")
    }

    fn route(path: &str) -> (u16, String) {
        match path {
            p if p.starts_with("/search") => (
                200,
                r#"<div class="result"><a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.test%2Fraft">Raft</a>
                   <a class="result__snippet">leader leases</a></div>"#
                    .into(),
            ),
            "/page" => (200, "<p>hello</p>".into()),
            "/big" => (200, "x".repeat(MAX_BODY_BYTES + 5000)),
            _ => (404, "nope".into()),
        }
    }

    #[tokio::test]
    async fn search_parses_hits_and_unwraps_redirect_urls() {
        let base = serve(route).await;
        let web = HttpWeb::with_search_url(format!("{base}/search"));
        let hits = web.search("raft").await.unwrap();
        assert_eq!(
            hits,
            vec![Hit {
                title: "Raft".into(),
                url: "https://example.test/raft".into(),
                snippet: "leader leases".into()
            }]
        );
    }

    #[tokio::test]
    async fn fetch_is_bounded_and_rejects_bad_schemes_and_statuses() {
        let base = serve(route).await;
        let web = HttpWeb::default();
        assert_eq!(
            web.fetch(&format!("{base}/page")).await.unwrap(),
            "<p>hello</p>"
        );
        assert_eq!(
            web.fetch(&format!("{base}/big")).await.unwrap().len(),
            MAX_BODY_BYTES
        );
        assert!(web
            .fetch(&format!("{base}/missing"))
            .await
            .unwrap_err()
            .contains("404"));
        assert!(web.fetch("file:///etc/passwd").await.is_err());
    }

    #[test]
    fn pdftotext_splits_pages_and_rejects_missing_files() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("two.pdf");
        std::fs::write(&path, two_page_pdf()).unwrap();
        let pages = PdftotextPdf.pages(path.to_str().unwrap()).unwrap();
        assert_eq!(pages.len(), 2);
        assert!(pages[0].contains("alpha") && pages[1].contains("beta"));
        assert!(PdftotextPdf.pages("/no/such.pdf").is_err());
    }

    /// Minimal two-page PDF; pdftotext rebuilds the missing xref table.
    fn two_page_pdf() -> String {
        let stream = |t: &str| format!("BT /F1 12 Tf 20 100 Td ({t}) Tj ET");
        let mut objs = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_owned(),
        ];
        for i in 0..2 {
            objs.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents {} 0 R /Resources << /Font << /F1 7 0 R >> >> >>",
                5 + i
            ));
        }
        for t in ["alpha", "beta"] {
            let s = stream(t);
            objs.push(format!("<< /Length {} >>\nstream\n{s}\nendstream", s.len()));
        }
        objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
        let mut pdf = String::from("%PDF-1.4\n");
        for (i, o) in objs.iter().enumerate() {
            pdf.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
        }
        pdf.push_str("trailer\n<< /Root 1 0 R /Size 8 >>\n%%EOF\n");
        pdf
    }
}
