use serde::Deserialize;
use std::collections::HashMap;

// Config related struct
#[derive(Deserialize)]
pub(crate) struct Config {
    #[serde(rename = "backup")]
    pub(crate) backups: Vec<Backup>,
}

#[derive(Clone, Deserialize, Debug)]
pub(crate) struct Backup {
    pub(crate) repository: String,
    pub(crate) password: String,
    pub(crate) options: HashMap<String, String>,
}
