//! The four sub-agents: each turns a tool-call brief into a result plus a file artifact.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use lp_cli_driver::engine::LlmEngine;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::agent::{run_submit_agent, str_arg, NoTools, Run, Tools};

const MAX_FETCH_CHARS: usize = 8_000;
const MAX_PAGE_CHARS: usize = 6_000;
const MAX_HITS: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Web access behind a port so tests never touch the network.
#[async_trait]
pub trait Web: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<Hit>, String>;
    /// Raw HTML (or text) of one page.
    async fn fetch(&self, url: &str) -> Result<String, String>;
}

/// PDF text extraction behind a port: one string per page.
pub trait Pdf: Send + Sync {
    fn pages(&self, path: &str) -> Result<Vec<String>, String>;
    fn page_count(&self, path: &str) -> Result<usize, String> {
        self.pages(path).map(|pages| pages.len())
    }
    fn page_range(&self, path: &str, first: usize, last: usize) -> Result<Vec<String>, String> {
        if first == 0 || last < first || last - first >= MAX_PDF_PAGES_PER_READ {
            return Err("Read 1 to 10 PDF pages at a time".into());
        }
        Ok(self
            .pages(path)?
            .into_iter()
            .skip(first - 1)
            .take(last - first + 1)
            .collect())
    }
}

pub const MAX_PDF_PAGES_PER_READ: usize = 10;
pub type PdfProgress = Arc<dyn Fn(usize, usize) + Send + Sync>;

/// A finished job: the `JobRecord.result` value and the artifact file (extension, contents).
pub struct Done {
    pub result: Value,
    pub artifact: (&'static str, String),
}

/// Strip scripts, styles and tags from HTML and collapse whitespace.
pub fn readable(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    // ASCII lowering keeps byte offsets, so `lower` is searched in step with `rest`.
    let lower = html.to_ascii_lowercase();
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let at = html.len() - rest.len();
        let raw = [("<script", "</script"), ("<style", "</style")]
            .into_iter()
            .find(|(open, _)| lower[at..].starts_with(open));
        if let Some((_, end)) = raw {
            rest = &rest[lower[at..].find(end).unwrap_or(rest.len())..];
        }
        rest = rest.find('>').map_or("", |j| &rest[j + 1..]);
        out.push(' ');
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cap(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn parse<T: for<'de> Deserialize<'de>>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| format!("invalid submit arguments: {e}"))
}

fn non_empty(what: &str, s: &str) -> Result<(), String> {
    if s.trim().is_empty() {
        Err(format!("{what} must be non-empty"))
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------- research

#[derive(Serialize, Deserialize)]
struct Source {
    title: String,
    url: String,
}

#[derive(Deserialize)]
struct ResearchSubmit {
    brief: String,
    sources: Vec<Source>,
}

/// Search and fetch, remembering which pages were actually fetched.
struct WebTools<'a>(&'a dyn Web, Mutex<Vec<String>>);

#[async_trait]
impl Tools for WebTools<'_> {
    async fn call(&self, name: &str, args: &Value) -> Option<Result<String, String>> {
        Some(match name {
            "search" => match str_arg(args, "query") {
                Ok(q) => self.0.search(q).await.map(|hits| {
                    hits.iter()
                        .take(MAX_HITS)
                        .map(|h| format!("{} | {} | {}", h.title, h.url, h.snippet))
                        .collect::<Vec<_>>()
                        .join("\n")
                }),
                Err(e) => Err(e),
            },
            "fetch" => match str_arg(args, "url") {
                Ok(u) => self.0.fetch(u).await.map(|h| {
                    self.1.lock().push(u.to_owned());
                    cap(&readable(&h), MAX_FETCH_CHARS)
                }),
                Err(e) => Err(e),
            },
            _ => return None,
        })
    }
}

pub async fn research<E: LlmEngine>(
    run: &Run<'_, E>,
    web: &dyn Web,
    args: &Value,
) -> Result<Done, String> {
    let question = str_arg(args, "question")?;
    let task = format!(
        "You are a research sub-agent. Answer: {question}\n\
         Tools: search {{query}} lists results as `title | url | snippet`; fetch {{url}} returns page text.\n\
         submit {{brief, sources:[{{title,url}}]}} with a concise brief and the sources it rests on."
    );
    let tools = WebTools(web, Mutex::default());
    let sub: ResearchSubmit = run_submit_agent(run, &task, &tools, |v| {
        let s: ResearchSubmit = parse(v)?;
        non_empty("brief", &s.brief)?;
        if s.sources.is_empty() || s.sources.iter().any(|x| x.url.trim().is_empty()) {
            return Err("sources must be non-empty and each needs a url".into());
        }
        Ok(s)
    })
    .await
    .map_err(|e| e.to_string())?;
    let mut md = format!("# {question}\n\n{}\n\n", sub.brief);
    for s in &sub.sources {
        md.push_str(&format!("- [{}]({})\n", s.title, s.url));
    }
    let fetched = tools.1.into_inner();
    md.push_str("\n## Fetched\n\n");
    for u in &fetched {
        md.push_str(&format!("- {u}\n"));
    }
    Ok(Done {
        result: json!({ "brief": sub.brief, "sources": sub.sources, "fetched": fetched }),
        artifact: ("md", md),
    })
}

// ---------------------------------------------------------------- pdf

#[derive(Deserialize)]
struct BriefSubmit {
    brief: String,
}

/// Page search and reads, remembering which pages the model actually saw.
struct PdfTools {
    reader: Arc<dyn Pdf>,
    path: String,
    first: usize,
    last: usize,
    total: usize,
    pages: Mutex<BTreeMap<usize, String>>,
    read: Mutex<BTreeSet<usize>>,
    consulted: Mutex<BTreeSet<usize>>,
    progress: PdfProgress,
}

impl PdfTools {
    async fn extract(&self, first: usize, last: usize) -> Result<Vec<(usize, String)>, String> {
        if first < self.first
            || last > self.last
            || last < first
            || last - first >= MAX_PDF_PAGES_PER_READ
        {
            return Err(format!(
                "Read up to 10 pages within {}..={}",
                self.first, self.last
            ));
        }
        if !(first..=last).all(|p| self.pages.lock().contains_key(&p)) {
            let reader = self.reader.clone();
            let path = self.path.clone();
            let pages = tokio::task::spawn_blocking(move || reader.page_range(&path, first, last))
                .await
                .map_err(|e| format!("PDF extraction stopped: {e}"))??;
            if pages.len() != last - first + 1 {
                return Err("PDF extractor returned an incomplete page range".into());
            }
            let mut cache = self.pages.lock();
            let bytes = cache.values().map(String::len).sum::<usize>()
                + pages.iter().map(String::len).sum::<usize>();
            if bytes > 8 << 20 {
                return Err(
                    "PDF text exceeds the reading budget; request a smaller page range".into(),
                );
            }
            cache.extend(
                pages
                    .into_iter()
                    .enumerate()
                    .map(|(i, text)| (first + i, text)),
            );
            (self.progress)(cache.len(), self.total);
        }
        let cache = self.pages.lock();
        Ok((first..=last).map(|p| (p, cache[&p].clone())).collect())
    }
}

#[async_trait]
impl Tools for PdfTools {
    async fn call(&self, name: &str, args: &Value) -> Option<Result<String, String>> {
        Some(match name {
            "search" => match str_arg(args, "query") {
                Ok(q) => {
                    let q = q.to_lowercase();
                    let mut out = Vec::new();
                    for first in (self.first..=self.last).step_by(MAX_PDF_PAGES_PER_READ) {
                        let pages = match self
                            .extract(first, (first + MAX_PDF_PAGES_PER_READ - 1).min(self.last))
                            .await
                        {
                            Ok(pages) => pages,
                            Err(e) => return Some(Err(e)),
                        };
                        for (p, text) in pages {
                            if let Some(line) = text.lines().find(|l| l.to_lowercase().contains(&q))
                            {
                                self.consulted.lock().insert(p);
                                out.push(format!("page {p}: {}", cap(line.trim(), 200)));
                                if out.len() == MAX_HITS {
                                    return Some(Ok(out.join("\n")));
                                }
                            }
                        }
                    }
                    Ok(out.join("\n"))
                }
                Err(e) => Err(e),
            },
            "read" => {
                let page = args
                    .get("page")
                    .and_then(Value::as_u64)
                    .and_then(|p| usize::try_from(p).ok());
                let end = match args.get("end_page") {
                    Some(value) => value.as_u64().and_then(|p| usize::try_from(p).ok()),
                    None => page,
                };
                match (page, end) {
                    (Some(first), Some(last)) => self.extract(first, last).await.map(|pages| {
                        let mut out = Vec::new();
                        for (p, text) in pages {
                            self.read.lock().insert(p);
                            self.consulted.lock().insert(p);
                            out.push(format!("page {p}: {}", cap(&text, MAX_PAGE_CHARS)));
                        }
                        out.join("\n")
                    }),
                    _ => Err("page must be a positive integer".into()),
                }
            }
            _ => return None,
        })
    }
}

pub async fn pdf<E: LlmEngine>(
    run: &Run<'_, E>,
    reader: Arc<dyn Pdf>,
    args: &Value,
    progress: PdfProgress,
) -> Result<Done, String> {
    let (path, question) = (str_arg(args, "path")?, str_arg(args, "question")?);
    let count_reader = reader.clone();
    let count_path = path.to_owned();
    let total = tokio::task::spawn_blocking(move || count_reader.page_count(&count_path))
        .await
        .map_err(|e| format!("PDF page count stopped: {e}"))??;
    let page_arg = |key: &str, default: usize| -> Result<usize, String> {
        match args.get(key) {
            None => Ok(default),
            Some(value) => value
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| format!("{key} must be a positive page number")),
        }
    };
    let first = page_arg("start_page", 1)?;
    let last = page_arg("end_page", total)?;
    if first == 0 || first > last || last > total {
        return Err(format!("PDF page range must be within 1..={total}"));
    }
    (progress)(0, total);
    let tools = PdfTools {
        reader,
        path: path.to_owned(),
        first,
        last,
        total,
        pages: Mutex::default(),
        read: Mutex::default(),
        consulted: Mutex::default(),
        progress,
    };
    let preview = tools
        .extract(first, (first + 2).min(last))
        .await?
        .into_iter()
        .map(|(p, text)| format!("page {p}: {}", cap(&text, 1200)))
        .collect::<Vec<_>>()
        .join("\n");
    let task = format!(
        "You are a PDF reading sub-agent for {path} ({total} pages; requested range {first}..={last}). Answer: {question}\n\
         First inspect this opening/TOC preview and identify relevant pages:\n{preview}\n\
         Tools: search {{query}} lists matching pages in the requested range; read {{page, end_page?}} returns at most 10 pages (1-based).\n\
         submit {{brief}} after reading the pages your answer rests on."
    );
    let sub: BriefSubmit = run_submit_agent(run, &task, &tools, |v| {
        let s: BriefSubmit = parse(v)?;
        non_empty("brief", &s.brief)?;
        if tools.read.lock().is_empty() {
            return Err("read at least one page before submitting".into());
        }
        Ok(s)
    })
    .await
    .map_err(|e| e.to_string())?;
    let (read, consulted) = (tools.read.lock().clone(), tools.consulted.lock().clone());
    let md = format!(
        "# {question}\n\n{}\n\nSource: {path}, pages read {read:?}\n",
        sub.brief
    );
    Ok(Done {
        result: json!({
            "path": path, "question": question, "brief": sub.brief,
            "pagesRead": read, "pagesConsulted": consulted,
            "progress": {"pagesRead": tools.pages.lock().len(), "totalPages": total},
            "startPage": first, "endPage": last,
        }),
        artifact: ("md", md),
    })
}

// ---------------------------------------------------------------- visual

#[derive(Deserialize)]
struct VisualSubmit {
    svg: String,
    ascii_fallback: String,
    notes: String,
}

pub async fn visual<E: LlmEngine>(run: &Run<'_, E>, args: &Value) -> Result<Done, String> {
    let description = str_arg(args, "description")?;
    let task = format!(
        "You are a diagram sub-agent. Draw: {description}\n\
         submit {{svg, ascii_fallback, notes}}: a self-contained <svg>, the same diagram in Unicode \
         box-drawing characters for the terminal, and short notes on how to read it."
    );
    let sub: VisualSubmit = run_submit_agent(run, &task, &NoTools, |v| {
        let s: VisualSubmit = parse(v)?;
        let svg = s.svg.trim();
        if !svg.starts_with("<svg") || !svg.ends_with("</svg>") || svg.contains("<script") {
            return Err("svg must be one script-free <svg>...</svg> element".into());
        }
        non_empty("ascii_fallback", &s.ascii_fallback)?;
        Ok(s)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(Done {
        result: json!({ "svg": sub.svg, "ascii_fallback": sub.ascii_fallback, "notes": sub.notes }),
        artifact: ("svg", sub.svg),
    })
}

// ---------------------------------------------------------------- demo

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slider {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
}

/// Terminal demo model: `frames[i]` is the curve with the first slider at its i-th of N even steps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TuiDemoSpec {
    pub title: String,
    pub sliders: Vec<Slider>,
    pub frames: Vec<Vec<f64>>,
}

#[derive(Deserialize)]
struct DemoSubmit {
    html: String,
    #[serde(flatten)]
    spec: TuiDemoSpec,
}

fn check_demo(s: &DemoSubmit) -> Result<(), String> {
    non_empty("title", &s.spec.title)?;
    if !s.html.contains("<html") || s.html.contains("http://") || s.html.contains("https://") {
        return Err("html must be one self-contained page with no external references".into());
    }
    if s.spec.sliders.is_empty()
        || s.spec
            .sliders
            .iter()
            .any(|v| !(v.min < v.max && (v.min..=v.max).contains(&v.default)))
    {
        return Err("each slider needs min < max and a default inside that range".into());
    }
    let width = s.spec.frames.first().map_or(0, Vec::len);
    if width == 0 || s.spec.frames.iter().any(|f| f.len() != width) {
        return Err("frames must be non-empty rows of equal, non-zero length".into());
    }
    Ok(())
}

pub async fn demo<E: LlmEngine>(run: &Run<'_, E>, args: &Value) -> Result<Done, String> {
    let description = str_arg(args, "description")?;
    let task = format!(
        "You are a demo-builder sub-agent. Build an interactive explorer of: {description}\n\
         submit {{title, html, sliders:[{{name,min,max,default}}], frames:[[y,...]]}}: a standalone HTML \
         page, and the terminal model where frames[i] is the curve with the first slider at its i-th of \
         N evenly spaced steps."
    );
    let sub: DemoSubmit = run_submit_agent(run, &task, &NoTools, |v| {
        let s: DemoSubmit = parse(v)?;
        check_demo(&s)?;
        Ok(s)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(Done {
        result: json!({ "spec": sub.spec }),
        artifact: ("html", sub.html),
    })
}
