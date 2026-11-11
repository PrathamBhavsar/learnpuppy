//! Textbook wire types (U7, `brain/learning-method/textbook-spec.md` §7.3). Built field by field
//! from the service's views, so no `answer`, `accept`, `why`, `mustName`, `misconceptions` or
//! `correct` can reach a chapter detail or a reading check.
//!
//! Plain-text rule: `title`, `error`, `q`, `options`, `answer`, `why` and objective `text` come
//! from a PDF or a model. They are served as JSON strings only; clients render them as text.

use lp_core::textbook::{ChapterKind, ChapterStatus, MapSource};
use lp_service::textbook as svc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::course::ItemKindDto;

/// A PDF book, at most 50 MB.
#[derive(ToSchema)]
#[allow(dead_code)]
pub struct TextbookUploadForm {
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextbookUploadDto {
    pub book_id: String,
    /// Always false after the upload returns: upload maps the book and starts no job. A chapter is
    /// imported when the learner opens it (`POST …/import` with that chapter).
    pub existing: bool,
    pub bytes: u64,
}

impl TextbookUploadDto {
    pub fn new(u: svc::Upload, bytes: u64) -> Self {
        Self {
            book_id: u.book,
            existing: u.existing,
            bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BookPhaseDto {
    Importing,
    Ready,
    Partial,
    Failed,
}

impl From<svc::BookPhase> for BookPhaseDto {
    fn from(p: svc::BookPhase) -> Self {
        match p {
            svc::BookPhase::Importing => Self::Importing,
            svc::BookPhase::Ready => Self::Ready,
            svc::BookPhase::Partial => Self::Partial,
            svc::BookPhase::Failed => Self::Failed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextbookSummaryDto {
    pub id: String,
    pub title: String,
    pub total_pages: u32,
    pub chapters: u32,
    pub ready: u32,
    pub state: BookPhaseDto,
    /// Epoch ms of the first import.
    pub created_at: i64,
}

impl From<svc::BookSummary> for TextbookSummaryDto {
    fn from(b: svc::BookSummary) -> Self {
        Self {
            id: b.id,
            title: b.title,
            total_pages: b.total_pages,
            chapters: b.chapters as u32,
            ready: b.ready as u32,
            state: b.state.into(),
            created_at: b.created_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct TextbookListDto {
    pub items: Vec<TextbookSummaryDto>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MapSourceDto {
    Outline,
    Toc,
    Chunks,
}

impl From<MapSource> for MapSourceDto {
    fn from(s: MapSource) -> Self {
        match s {
            MapSource::Outline => Self::Outline,
            MapSource::Toc => Self::Toc,
            MapSource::Chunks => Self::Chunks,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChapterKindDto {
    Chapter,
    Front,
    Back,
}

impl From<ChapterKind> for ChapterKindDto {
    fn from(k: ChapterKind) -> Self {
        match k {
            ChapterKind::Chapter => Self::Chapter,
            ChapterKind::Front => Self::Front,
            ChapterKind::Back => Self::Back,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChapterStateDto {
    Pending,
    Importing,
    Ready,
    Failed,
    Skipped,
}

impl From<svc::ChapterRowState> for ChapterStateDto {
    fn from(s: svc::ChapterRowState) -> Self {
        match s {
            svc::ChapterRowState::Pending => Self::Pending,
            svc::ChapterRowState::Importing => Self::Importing,
            svc::ChapterRowState::Ready => Self::Ready,
            svc::ChapterRowState::Failed => Self::Failed,
            svc::ChapterRowState::Skipped => Self::Skipped,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChapterStatusDto {
    NotRead,
    Checked,
    Due,
}

impl From<ChapterStatus> for ChapterStatusDto {
    fn from(s: ChapterStatus) -> Self {
        match s {
            ChapterStatus::NotRead => Self::NotRead,
            ChapterStatus::Checked => Self::Checked,
            ChapterStatus::Due => Self::Due,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChapterRowDto {
    pub index: u32,
    pub title: String,
    /// Physical, 1-based.
    pub start_page: u32,
    pub end_page: u32,
    pub kind: ChapterKindDto,
    pub state: ChapterStateDto,
    pub status: ChapterStatusDto,
    pub objectives: u32,
    /// A short code (`parse`, `timeout`, `no_cards`, `engine_unavailable`, …); never page text.
    pub error: Option<String>,
    /// The chapter was longer than one call may carry and its tail was cut.
    pub truncated: bool,
    /// Importing it may start now: not imported yet, or failed with its retry unused.
    pub can_import: bool,
}

impl From<svc::ChapterRow> for ChapterRowDto {
    fn from(c: svc::ChapterRow) -> Self {
        Self {
            index: c.index,
            title: c.title,
            start_page: c.pages.start,
            end_page: c.pages.end,
            kind: c.kind.into(),
            state: c.state.into(),
            status: c.status.into(),
            objectives: c.objectives as u32,
            error: c.error,
            truncated: c.truncated,
            can_import: c.can_import,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextbookDto {
    pub id: String,
    pub title: String,
    pub file_name: String,
    pub total_pages: u32,
    /// Physical minus printed page number, when one was found.
    pub page_offset: Option<i32>,
    pub map_source: Option<MapSourceDto>,
    /// The running import job.
    pub job_id: Option<String>,
    pub auto_calls: u32,
    pub requested_calls: u32,
    /// Chapter imports in the last hour, and the most a book allows in any rolling hour.
    pub hour_used: u32,
    pub hour_cap: u32,
    pub chapters: Vec<ChapterRowDto>,
}

impl From<svc::BookView> for TextbookDto {
    fn from(b: svc::BookView) -> Self {
        Self {
            id: b.meta.id,
            title: b.meta.title,
            file_name: b.meta.file_name,
            total_pages: b.meta.total_pages,
            page_offset: b.meta.page_offset,
            map_source: b.source.map(Into::into),
            job_id: b.job,
            auto_calls: b.auto_calls,
            requested_calls: b.requested_calls,
            hour_used: b.hour_used,
            hour_cap: lp_core::textbook::MAX_MANUAL_IMPORTS_PER_HOUR,
            chapters: b.chapters.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveRowDto {
    pub id: String,
    pub text: String,
    pub start_page: u32,
    pub end_page: u32,
    /// How many stored quiz items; the items themselves are never sent here.
    pub items: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct ChapterDetailDto {
    pub chapter: ChapterRowDto,
    pub objectives: Vec<ObjectiveRowDto>,
}

impl From<svc::ChapterDetail> for ChapterDetailDto {
    fn from(d: svc::ChapterDetail) -> Self {
        Self {
            chapter: d.row.into(),
            objectives: d
                .objectives
                .into_iter()
                .map(|o| ObjectiveRowDto {
                    id: o.id,
                    text: o.text,
                    start_page: o.pages.start,
                    end_page: o.pages.end,
                    items: o.items as u32,
                })
                .collect(),
        }
    }
}

/// At most 200 chapter indices; omit to import everything still pending.
#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub chapters: Option<Vec<u32>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportStartedDto {
    pub job_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckItemDto {
    pub id: String,
    pub objective_id: String,
    pub kind: ItemKindDto,
    pub q: String,
    /// Multiple choice only, shuffled; empty for recall.
    pub options: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingCheckDto {
    pub check_id: String,
    pub chapter: u32,
    pub items: Vec<CheckItemDto>,
}

impl From<svc::CheckView> for ReadingCheckDto {
    fn from(c: svc::CheckView) -> Self {
        Self {
            check_id: c.check,
            chapter: c.chapter,
            items: c
                .items
                .into_iter()
                .map(|i| CheckItemDto {
                    id: i.id,
                    objective_id: i.objective,
                    kind: i.kind.into(),
                    q: i.q,
                    options: i.options,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckAnswerRequest {
    pub item: String,
    pub text: String,
    /// 1..=5.
    pub confidence: Option<u8>,
    /// How long the learner took, measured by the client.
    pub ms: Option<u64>,
    /// A retry with the same id returns the stored feedback instead of `409`.
    pub client_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckFeedbackDto {
    pub correct: bool,
    /// Near the right answer: the learner confirms with a self-grade.
    pub close: bool,
    pub answer: String,
    pub why: String,
    /// Epoch ms the card is next due.
    pub due: Option<i64>,
}

impl From<svc::CheckFeedback> for CheckFeedbackDto {
    fn from(f: svc::CheckFeedback) -> Self {
        Self {
            correct: f.correct,
            close: f.close,
            answer: f.answer,
            why: f.why,
            due: f.due,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextbookUsageDto {
    pub cheap_in: u64,
    pub cheap_out: u64,
    pub auto_calls: u32,
    pub requested_calls: u32,
    /// Chapter imports in the last hour, and the most a book allows in any rolling hour.
    pub hour_used: u32,
    pub hour_cap: u32,
    /// Epoch ms when the oldest counted import leaves the hour; set only while at the cap.
    pub next_slot_at: Option<i64>,
}

impl From<svc::TextbookUsage> for TextbookUsageDto {
    fn from(u: svc::TextbookUsage) -> Self {
        Self {
            cheap_in: u.cheap_in,
            cheap_out: u.cheap_out,
            auto_calls: u.auto_calls,
            requested_calls: u.requested_calls,
            hour_used: u.hour_used,
            hour_cap: u.hour_cap,
            next_slot_at: u.next_slot_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_core::course::ItemKind;
    use lp_core::textbook::PageRange;
    use serde_json::{json, Value};

    fn row() -> svc::ChapterRow {
        svc::ChapterRow {
            index: 2,
            title: "<img src=x onerror=1>".into(),
            pages: PageRange { start: 5, end: 9 },
            kind: ChapterKind::Chapter,
            state: svc::ChapterRowState::Pending,
            objectives: 7,
            status: ChapterStatus::Due,
            error: Some("parse".into()),
            truncated: true,
            can_import: true,
        }
    }

    fn keys(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    out.push(k.clone());
                    keys(v, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|v| keys(v, out)),
            _ => {}
        }
    }

    #[test]
    fn upload_and_list_round_trip() {
        let v = serde_json::to_value(TextbookUploadDto::new(
            svc::Upload {
                book: "tb_0123456789abcdef".into(),
                existing: true,
            },
            12,
        ))
        .unwrap();
        assert_eq!(
            v,
            json!({"bookId":"tb_0123456789abcdef","existing":true,"bytes":12})
        );
        let l = TextbookListDto {
            items: vec![svc::BookSummary {
                id: "tb_x".into(),
                title: "T".into(),
                total_pages: 9,
                chapters: 3,
                ready: 2,
                state: svc::BookPhase::Partial,
                created_at: 5,
            }
            .into()],
        };
        assert_eq!(
            serde_json::to_value(l).unwrap(),
            json!({"items":[{"id":"tb_x","title":"T","totalPages":9,"chapters":3,"ready":2,
                "state":"partial","createdAt":5}]})
        );
    }

    #[test]
    fn book_and_chapter_round_trip() {
        let r = serde_json::to_value(ChapterRowDto::from(row())).unwrap();
        assert_eq!(
            r,
            json!({"index":2,"title":"<img src=x onerror=1>","startPage":5,"endPage":9,
                "kind":"chapter","state":"pending","status":"due","objectives":7,
                "error":"parse","truncated":true,"canImport":true})
        );
        let book = TextbookDto::from(svc::BookView {
            meta: lp_core::textbook::BookMeta {
                id: "tb_0123456789abcdef".into(),
                title: "T".into(),
                file_name: "t.pdf".into(),
                bytes: 1,
                sha256: "0".repeat(64),
                total_pages: 9,
                page_offset: Some(-2),
            },
            source: Some(MapSource::Toc),
            chapters: vec![row()],
            job: Some("tbj_1".into()),
            auto_calls: 3,
            requested_calls: 1,
            hour_used: 2,
        });
        let v = serde_json::to_value(book).unwrap();
        assert_eq!(v["mapSource"], "toc");
        assert_eq!(v["pageOffset"], -2);
        assert_eq!(
            (v["hourUsed"].as_u64(), v["hourCap"].as_u64()),
            (Some(2), Some(8))
        );
        assert!(v.get("callsCap").is_none());
        assert_eq!(v["jobId"], "tbj_1");
        assert_eq!(v["chapters"][0]["startPage"], 5);
        // titles stay byte-identical: no escaping on the server
        assert_eq!(v["chapters"][0]["title"], "<img src=x onerror=1>");
        for f in ["fileName", "totalPages", "autoCalls", "requestedCalls"] {
            assert!(v.get(f).is_some(), "{f}");
        }
    }

    #[test]
    fn detail_check_feedback_usage_and_requests_round_trip() {
        let d = ChapterDetailDto::from(svc::ChapterDetail {
            row: row(),
            objectives: vec![svc::ObjectiveRow {
                id: "ch002-o01".into(),
                text: "Know x".into(),
                pages: PageRange { start: 5, end: 6 },
                items: 2,
            }],
        });
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["objectives"][0]["startPage"], 5);
        assert_eq!(v["objectives"][0]["items"], 2);
        let c = ReadingCheckDto::from(svc::CheckView {
            check: "rc_1".into(),
            chapter: 2,
            items: vec![svc::CheckItem {
                id: "ch002-o01-i1".into(),
                objective: "ch002-o01".into(),
                kind: ItemKind::Mcq,
                q: "Which?".into(),
                options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            }],
        });
        let cv = serde_json::to_value(&c).unwrap();
        assert_eq!(cv["checkId"], "rc_1");
        assert_eq!(cv["items"][0]["objectiveId"], "ch002-o01");
        assert_eq!(cv["items"][0]["kind"], "mcq");
        let f = serde_json::to_value(CheckFeedbackDto::from(svc::CheckFeedback {
            correct: true,
            close: false,
            answer: "a".into(),
            why: "w".into(),
            due: Some(7),
        }))
        .unwrap();
        assert_eq!(
            f,
            json!({"correct":true,"close":false,"answer":"a","why":"w","due":7})
        );
        let u = serde_json::to_value(TextbookUsageDto::from(svc::TextbookUsage {
            cheap_in: 1,
            cheap_out: 2,
            auto_calls: 3,
            requested_calls: 4,
            hour_used: 8,
            hour_cap: 8,
            next_slot_at: Some(99),
        }))
        .unwrap();
        assert_eq!(
            u,
            json!({"cheapIn":1,"cheapOut":2,"autoCalls":3,"requestedCalls":4,
                "hourUsed":8,"hourCap":8,"nextSlotAt":99})
        );
        assert_eq!(
            serde_json::to_value(ImportStartedDto { job_id: "j".into() }).unwrap(),
            json!({"jobId":"j"})
        );
        let req: CheckAnswerRequest = serde_json::from_value(
            json!({"item":"i","text":"t","confidence":3,"ms":9,"clientId":"c"}),
        )
        .unwrap();
        assert_eq!(
            (req.confidence, req.ms, req.client_id.as_deref()),
            (Some(3), Some(9), Some("c"))
        );
        let imp: ImportRequest = serde_json::from_value(json!({"chapters":[1,2]})).unwrap();
        assert_eq!(imp.chapters, Some(vec![1, 2]));
        assert!(serde_json::from_value::<ImportRequest>(json!({}))
            .unwrap()
            .chapters
            .is_none());
    }

    /// `ReadingCheckDto` and `ChapterDetailDto` never carry the answer key at any depth.
    #[test]
    fn reading_check_dto_has_no_answer_fields() {
        let d = serde_json::to_value(ChapterDetailDto::from(svc::ChapterDetail {
            row: row(),
            objectives: vec![svc::ObjectiveRow {
                id: "o".into(),
                text: "t".into(),
                pages: PageRange { start: 1, end: 1 },
                items: 1,
            }],
        }))
        .unwrap();
        let c = serde_json::to_value(ReadingCheckDto::from(svc::CheckView {
            check: "rc_1".into(),
            chapter: 0,
            items: vec![svc::CheckItem {
                id: "i".into(),
                objective: "o".into(),
                kind: ItemKind::Recall,
                q: "q".into(),
                options: vec![],
            }],
        }))
        .unwrap();
        let mut all = Vec::new();
        keys(&d, &mut all);
        keys(&c, &mut all);
        assert!(!all.is_empty());
        for banned in [
            "answer",
            "accept",
            "why",
            "mustName",
            "must_name",
            "misconceptions",
            "correct",
        ] {
            assert!(!all.iter().any(|k| k == banned), "{banned} in {all:?}");
        }
    }
}
