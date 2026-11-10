//! Real `Web` and `Pdf` adapters for the workbench, bounded by timeouts and size caps.

use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use scraper::{Html, Selector};

use super::jobs::{Hit, Pdf, Web};

const SEARCH_URL: &str = "https://html.duckduckgo.com/html/";
const WEB_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_BODY_BYTES: usize = 2 << 20;
const MAX_REDIRECTS: usize = 4;
const MAX_HITS: usize = 20;
const PDF_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PDF_BYTES: u64 = 8 << 20;
const MAX_PDF_INPUT_BYTES: u64 = 40 << 20;
const PDF_STALL_TIMEOUT: Duration = Duration::from_secs(10);

/// Search through DuckDuckGo's HTML endpoint and fetch pages over http(s).
///
/// URLs come from an LLM, so every request is SSRF-checked: redirects are followed by hand and
/// each hop is re-validated, and the connection is pinned to the address that passed the check.
pub struct HttpWeb {
    search_url: String,
    allow_loopback: bool,
    /// Test-only: answer every DNS lookup with this address instead of asking the resolver.
    #[cfg(test)]
    fake_dns: Option<SocketAddr>,
}

impl Default for HttpWeb {
    fn default() -> Self {
        Self {
            search_url: SEARCH_URL.into(),
            allow_loopback: false,
            #[cfg(test)]
            fake_dns: None,
        }
    }
}

impl HttpWeb {
    /// Test-only: point search at a local mock server and let it be reached on loopback.
    #[cfg(test)]
    fn for_test(url: impl Into<String>) -> Self {
        Self {
            search_url: url.into(),
            allow_loopback: true,
            fake_dns: None,
        }
    }

    /// GET `url`, following up to `MAX_REDIRECTS` redirects, reading at most `MAX_BODY_BYTES`.
    async fn get(&self, mut url: reqwest::Url) -> Result<String, String> {
        for _ in 0..=MAX_REDIRECTS {
            let mut builder = reqwest::Client::builder()
                .timeout(WEB_TIMEOUT)
                .user_agent("learnpuppy")
                // A proxy would resolve and connect on our behalf, bypassing the vetted pin.
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none());
            if let Some((host, addr)) = self.vet(&mut url).await? {
                builder = builder.resolve(&host, addr);
            }
            let client = builder.build().map_err(|e| e.to_string())?;
            let mut resp = client
                .get(url.clone())
                .send()
                .await
                .map_err(|e| e.without_url().to_string())?;
            if resp.status().is_redirection() {
                let next = resp
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or("redirect without location")?;
                url = url.join(next).map_err(|e| format!("bad redirect: {e}"))?;
                continue;
            }
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
            return Ok(String::from_utf8_lossy(&buf).into_owned());
        }
        Err("too many redirects".into())
    }

    /// Reject URLs that could reach internal hosts. Returns the host and the validated address
    /// to pin the connection to, or `None` for an IP-literal host (nothing to resolve). The URL's
    /// host is rewritten to the normalised string so reqwest's pin lookup matches it exactly.
    async fn vet(&self, url: &mut reqwest::Url) -> Result<Option<(String, SocketAddr)>, String> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err("only http(s) urls can be fetched".into());
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("urls with credentials are not allowed".into());
        }
        let blocked = |ip: IpAddr| is_internal(ip) && !(self.allow_loopback && ip.is_loopback());
        let port = url.port_or_known_default().ok_or("url has no port")?;
        let host = url.host_str().ok_or("url has no host")?;
        if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
            return if blocked(ip) {
                Err(format!("blocked address {ip}"))
            } else {
                Ok(None)
            };
        }
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        if !self.allow_loopback && (host == "localhost" || host.ends_with(".localhost")) {
            return Err("blocked host localhost".into());
        }
        url.set_host(Some(&host)).map_err(|e| e.to_string())?;
        #[cfg(test)]
        let fake = self.fake_dns;
        #[cfg(not(test))]
        let fake: Option<SocketAddr> = None;
        let addrs: Vec<SocketAddr> = match fake {
            Some(a) => vec![a],
            None => tokio::net::lookup_host((host.as_str(), port))
                .await
                .map_err(|e| format!("dns lookup failed: {e}"))?
                .collect(),
        };
        if let Some(a) = addrs.iter().find(|a| blocked(a.ip())) {
            return Err(format!("{host} resolves to blocked address {}", a.ip()));
        }
        let addr = *addrs.first().ok_or("host did not resolve")?;
        Ok(Some((host, addr)))
    }
}

/// True for loopback, unspecified, private, link-local, unique-local, multicast and broadcast
/// addresses, including IPv4-mapped IPv6 forms of them.
fn is_internal(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_unspecified()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.octets()[0] == 0 // 0.0.0.0/8
                || (v4.octets()[0] == 100 && v4.octets()[1] & 0xc0 == 0x40) // 100.64.0.0/10 CGNAT
                || v4.octets()[..3] == [192, 0, 0] // 192.0.0.0/24
                || (v4.octets()[0] == 198 && v4.octets()[1] & 0xfe == 18) // 198.18.0.0/15 benchmarking
                || v4.octets()[0] >= 240 // 240.0.0.0/4 reserved
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4() {
                return is_internal(v4.into()); // mapped and IPv4-compatible forms
            }
            let seg = v6.segments();
            if seg[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
                // 64:ff9b::/96 NAT64 embeds an IPv4 address
                let [a, b] = seg[6].to_be_bytes();
                let [c, d] = seg[7].to_be_bytes();
                return is_internal(std::net::Ipv4Addr::new(a, b, c, d).into());
            }
            let first = seg[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || first & 0xfe00 == 0xfc00 // fc00::/7 unique-local
                || first & 0xffc0 == 0xfe80 // fe80::/10 link-local
        }
    }
}

#[async_trait]
impl Web for HttpWeb {
    async fn search(&self, query: &str) -> Result<Vec<Hit>, String> {
        let mut url = reqwest::Url::parse(&self.search_url).map_err(|e| format!("bad url: {e}"))?;
        url.query_pairs_mut().append_pair("q", query);
        Ok(parse_hits(&self.get(url).await?))
    }

    async fn fetch(&self, url: &str) -> Result<String, String> {
        let url = reqwest::Url::parse(url).map_err(|e| format!("bad url: {e}"))?;
        self.get(url).await
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

fn pdf_path(path: &str) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Cannot open PDF: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{path} is not a file"));
    }
    if meta.len() > MAX_PDF_INPUT_BYTES {
        return Err("This PDF exceeds 40 MiB. Split or compress it first; scanned PDFs may need OCR before reading.".into());
    }
    // A leading `-` must not be read as an option.
    Ok(if path.starts_with('-') {
        format!("./{path}")
    } else {
        path.to_owned()
    })
}

fn pdf_command(
    command: &mut Command,
    timeout: Duration,
    stall: Duration,
) -> Result<Vec<u8>, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Poppler (pdfinfo and pdftotext) is required to read PDFs: {e}"))?;
    let mut out = child.stdout.take().ok_or("PDF extractor gave no output")?;
    let activity = Arc::new(Mutex::new(Instant::now()));
    let clock = activity.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = (|| {
            let mut buf = Vec::new();
            let mut chunk = [0; 8192];
            loop {
                let n = out.read(&mut chunk).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                if let Ok(mut last) = clock.lock() {
                    *last = Instant::now();
                }
                if buf.len() + n > MAX_PDF_BYTES as usize {
                    return Err(
                        "PDF page range produced too much text; request fewer pages".to_owned()
                    );
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Ok(buf)
        })();
        let _ = tx.send(result);
    });
    let start = Instant::now();
    let result = loop {
        match rx.try_recv() {
            Ok(Err(e)) => break Err(e),
            Ok(Ok(buf)) => {
                // EOF can precede process exit; keep watching the same deadline.
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) => break,
                        Ok(None)
                            if start.elapsed() < timeout
                                && activity.lock().is_ok_and(|last| last.elapsed() < stall) =>
                        {
                            std::thread::sleep(Duration::from_millis(20))
                        }
                        Ok(None) => break,
                        Err(e) => {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err(e.to_string());
                        }
                    }
                }
                break match child.try_wait() {
                    Ok(Some(status)) if status.success() => Ok(buf),
                    Ok(Some(_)) => Err(
                        "PDF extraction failed; check the file is readable and not encrypted"
                            .into(),
                    ),
                    _ => Err("PDF extraction timed out; try a smaller page range".into()),
                };
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                break Err("PDF extractor reader stopped".into())
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if start.elapsed() >= timeout {
            break Err("PDF extraction timed out; try a smaller page range".into());
        }
        if activity.lock().map_or(true, |last| last.elapsed() >= stall) {
            break Err("PDF extraction stalled without progress; try a smaller page range or OCR a scanned PDF".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = child.kill();
    let _ = child.wait();
    result
}

impl Pdf for PdftotextPdf {
    fn pages(&self, path: &str) -> Result<Vec<String>, String> {
        self.page_range(path, 1, super::jobs::MAX_PDF_PAGES_PER_READ)
    }

    fn page_count(&self, path: &str) -> Result<usize, String> {
        let arg = pdf_path(path)?;
        let buf = pdf_command(
            Command::new("pdfinfo").env("LC_ALL", "C").arg(arg),
            PDF_TIMEOUT,
            PDF_STALL_TIMEOUT,
        )?;
        String::from_utf8_lossy(&buf)
            .lines()
            .find_map(|line| {
                line.strip_prefix("Pages:")
                    .and_then(|n| n.trim().parse().ok())
            })
            .filter(|n| *n > 0)
            .ok_or_else(|| "Cannot determine PDF page count".into())
    }

    fn page_range(&self, path: &str, first: usize, last: usize) -> Result<Vec<String>, String> {
        if first == 0 || last < first || last - first >= super::jobs::MAX_PDF_PAGES_PER_READ {
            return Err("Read 1 to 10 PDF pages at a time, using 1-based page numbers".into());
        }
        let arg = pdf_path(path)?;
        let buf = pdf_command(
            Command::new("pdftotext").args([
                "-f",
                &first.to_string(),
                "-l",
                &last.to_string(),
                &arg,
                "-",
            ]),
            PDF_TIMEOUT,
            PDF_STALL_TIMEOUT,
        )?;
        let mut pages: Vec<String> = String::from_utf8_lossy(&buf)
            .split('\x0c')
            .map(str::to_owned)
            .collect();
        if pages.last().is_some_and(|p| p.trim().is_empty()) {
            pages.pop();
        }
        if pages.iter().all(|p| p.trim().is_empty()) {
            return Err(
                "No text found in this PDF range. Scanned pages need OCR before reading.".into(),
            );
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
        let web = HttpWeb::for_test(format!("{base}/search"));
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
        let web = HttpWeb::for_test("");
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

    #[tokio::test]
    async fn fetch_rejects_internal_targets() {
        let web = HttpWeb::default();
        for url in [
            "http://localhost/",
            "http://localhost./",
            "http://LOCALHOST:8080/",
            "http://127.0.0.1/",
            "http://[::1]/",
            "http://10.1.2.3/",
            "http://172.16.0.1/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://0.0.0.0/",
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:169.254.169.254]/",
            "http://[fd00::1]/",
            "http://[fe80::1]/",
            "http://user:pw@example.com/",
            "http://100.64.0.1/",
            "http://198.18.0.1/",
            "http://240.0.0.1/",
            "http://[::127.0.0.1]/",
        ] {
            assert!(web.fetch(url).await.is_err(), "{url} should be rejected");
        }
    }

    #[test]
    fn is_internal_covers_cgnat_zero_net_ietf_and_nat64() {
        for ip in [
            "100.100.100.200",
            "100.64.0.1",
            "100.127.255.255",
            "0.1.2.3",
            "192.0.0.8",
            "64:ff9b::7f00:1",
            "64:ff9b::a9fe:a9fe",
            "198.18.0.1",
            "198.19.255.255",
            "240.0.0.1",
            "255.255.255.255",
            "::127.0.0.1",
        ] {
            assert!(is_internal(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "100.63.0.1",
            "100.128.0.1",
            "192.0.1.1",
            "198.17.0.1",
            "198.20.0.1",
            "64:ff9b::808:808",
        ] {
            assert!(!is_internal(ip.parse().unwrap()), "{ip}");
        }
    }

    #[tokio::test]
    async fn pin_is_used_for_trailing_dot_and_mixed_case_hosts() {
        let base = serve(route).await;
        let addr: SocketAddr = base.trim_start_matches("http://").parse().unwrap();
        let mut web = HttpWeb::for_test("");
        web.fake_dns = Some(addr);
        // `pin.test` never resolves for real, so success proves the vetted address was used.
        for host in ["pin.test.", "PIN.Test", "Pin.TEST."] {
            let url = format!("http://{host}:{}/page", addr.port());
            assert_eq!(web.fetch(&url).await.unwrap(), "<p>hello</p>", "{url}");
        }
    }

    #[tokio::test]
    async fn redirect_to_private_address_is_rejected() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                let mut buf = vec![0; 4096];
                let _ = s.read(&mut buf).await;
                let head = "HTTP/1.1 302 X\r\nlocation: http://169.254.169.254/latest\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
                let _ = s.write_all(head.as_bytes()).await;
            }
        });
        // Loopback is allowed for the first hop only so the mock is reachable; the hop to the
        // metadata address must still be refused.
        let web = HttpWeb::for_test("");
        let err = web.fetch(&format!("http://{addr}/")).await.unwrap_err();
        assert!(err.contains("blocked"), "{err}");
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
        assert_eq!(PdftotextPdf.page_count(path.to_str().unwrap()).unwrap(), 2);
        let second = PdftotextPdf
            .page_range(path.to_str().unwrap(), 2, 2)
            .unwrap();
        assert_eq!(second.len(), 1);
        assert!(second[0].contains("beta") && !second[0].contains("alpha"));
        assert!(PdftotextPdf
            .page_range(path.to_str().unwrap(), 1, 11)
            .is_err());
        assert!(PdftotextPdf
            .page_range(path.to_str().unwrap(), 0, 1)
            .is_err());
    }

    #[test]
    fn pdf_rejects_oversized_input_before_starting_extractor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scanned.pdf");
        std::fs::File::create(&path)
            .unwrap()
            .set_len(MAX_PDF_INPUT_BYTES + 1)
            .unwrap();
        let error = PdftotextPdf.page_count(path.to_str().unwrap()).unwrap_err();
        assert!(error.contains("40 MiB") && error.contains("OCR"), "{error}");
    }

    #[test]
    fn pdf_subprocess_stall_and_timeout_are_bounded() {
        let start = Instant::now();
        let error = pdf_command(
            Command::new("sh").args(["-c", "exec sleep 3"]),
            Duration::from_secs(2),
            Duration::from_millis(40),
        )
        .unwrap_err();
        assert!(error.contains("stalled"), "{error}");
        assert!(start.elapsed() < Duration::from_secs(1));
        let error = pdf_command(
            Command::new("sh").args(["-c", "exec sleep 3"]),
            Duration::from_millis(40),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert!(error.contains("timed out"), "{error}");
    }

    /// Opt-in check against a PDF a real tool produced: `LP_REAL_PDF=/path cargo test -p lp-agent real_pdf -- --ignored`.
    #[test]
    #[ignore = "needs LP_REAL_PDF"]
    fn real_pdf_reads_by_range() {
        let path = std::env::var("LP_REAL_PDF").expect("set LP_REAL_PDF");
        let total = PdftotextPdf.page_count(&path).unwrap();
        assert!(total > 10, "use a PDF over ten pages");
        let pages = PdftotextPdf.page_range(&path, 11, 20).unwrap();
        assert_eq!(pages.len(), 10);
        assert!(pages.iter().all(|p| p.trim().len() > 50));
        assert!(PdftotextPdf.page_range(&path, 1, 11).is_err());
    }

    #[test]
    fn generated_large_pdf_extracts_only_requested_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("many.pdf");
        std::fs::write(&path, multi_page_pdf(25)).unwrap();
        let path = path.to_str().unwrap();
        assert_eq!(PdftotextPdf.page_count(path).unwrap(), 25);
        let pages = PdftotextPdf.page_range(path, 11, 20).unwrap();
        assert_eq!(pages.len(), 10);
        assert!(pages.first().unwrap().contains("page11"));
        assert!(pages.last().unwrap().contains("page20"));
        assert!(!pages.join("").contains("page21"));
    }

    fn two_page_pdf() -> String {
        multi_page_pdf(2)
    }

    /// Minimal PDF; Poppler rebuilds the missing xref table.
    fn multi_page_pdf(count: usize) -> String {
        let stream = |t: &str| format!("BT /F1 12 Tf 20 100 Td ({t}) Tj ET");
        let kids = (0..count)
            .map(|i| format!("{} 0 R", i + 3))
            .collect::<Vec<_>>()
            .join(" ");
        let mut objs = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            format!("<< /Type /Pages /Kids [{kids}] /Count {count} >>"),
        ];
        for i in 0..count {
            objs.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents {} 0 R /Resources << /Font << /F1 {} 0 R >> >> >>",
                count + 3 + i, count * 2 + 3
            ));
        }
        for i in 0..count {
            let text = match i {
                0 => "alpha".to_owned(),
                1 => "beta".to_owned(),
                _ => format!("page{}", i + 1),
            };
            let s = stream(&text);
            objs.push(format!("<< /Length {} >>\nstream\n{s}\nendstream", s.len()));
        }
        objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
        let mut pdf = String::from("%PDF-1.4\n");
        for (i, o) in objs.iter().enumerate() {
            pdf.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
        }
        pdf.push_str(&format!(
            "trailer\n<< /Root 1 0 R /Size {} >>\n%%EOF\n",
            count * 2 + 4
        ));
        pdf
    }
}
