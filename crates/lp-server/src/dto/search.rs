//! Global search wire types (docs/web/03 §Bootstrap / global).

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::graph::NodeBriefDto;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    pub q: String,
    /// Hits to return in total; default 30, at most 100.
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub workspace_id: String,
    pub workspace_name: String,
    pub node: NodeBriefDto,
    /// `0..=1`, higher is better; comparable across workspaces.
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchDto {
    /// Best match first.
    pub items: Vec<SearchHitDto>,
    /// Workspaces that did not answer in time; their hits are missing.
    pub timed_out: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_are_camel_case() {
        let hit = SearchHitDto {
            workspace_id: "ws_a".into(),
            workspace_name: "A".into(),
            node: NodeBriefDto {
                id: "n1".into(),
                name: "N".into(),
                text: "t".into(),
                kind: "chat".into(),
            },
            score: 0.5,
        };
        let j = serde_json::to_value(&hit).unwrap();
        assert_eq!(j["workspaceId"], "ws_a");
        assert_eq!(serde_json::from_value::<SearchHitDto>(j).unwrap(), hit);
    }
}
