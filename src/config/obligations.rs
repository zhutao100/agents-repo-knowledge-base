#[derive(Clone, Debug, serde::Deserialize)]
pub struct ObligationsConfig {
    #[serde(default)]
    pub rule: Vec<ObligationRule>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationRule {
    pub id: String,
    pub when_path_prefix: String,

    #[serde(default)]
    pub require_module_card: Option<String>,

    #[serde(default)]
    pub require_fact_types: Option<Vec<String>>,

    #[serde(default)]
    pub require_session_capsule: Option<bool>,
}
