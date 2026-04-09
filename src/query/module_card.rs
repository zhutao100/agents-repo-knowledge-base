#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleCardToml {
    pub id: String,
    pub title: String,

    #[serde(default)]
    pub owners: Vec<String>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub entrypoints: Vec<String>,

    #[serde(default)]
    pub edit_points: Vec<String>,

    #[serde(default)]
    pub related_facts: Vec<String>,
}
