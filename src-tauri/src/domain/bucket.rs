use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BucketInfo {
    pub name: String,
    pub created_at: Option<String>,
}
