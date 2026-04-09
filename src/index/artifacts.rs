#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct KbMeta {
    pub kb_format_version: u32,
    pub schemas: Vec<KbSchema>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct KbSchema {
    pub name: String,
    pub version: u32,
    pub required: bool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct TreeRecord {
    pub path: String,
    pub kind: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_symbols: Option<Vec<String>>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SymbolRecord {
    pub symbol_id: String,
    pub lang: String,
    pub path: String,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub line: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct DepEdge {
    pub from_path: String,
    pub kind: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_external: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}
