use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::E;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Metadata {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub url: String,
    pub license: String,
    pub audio: String,
    #[serde(default)]
    pub skip: u32,
    pub native: String,
    pub transcript: Option<String>,
    pub translations: HashMap<String, String>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    pub enclosing_directory: String,
}

impl Metadata {
    pub fn from_filename(filename: String) -> E<Self> {
        let f = std::fs::File::open(&filename)?;
        let reader = std::io::BufReader::new(f);
        let mut metadata: Self = serde_json::from_reader(reader)?;
        log::debug!("metadata::from_filename: {:?}", metadata);
        metadata.enclosing_directory = Path::parent(Path::new(&filename))
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        Ok(metadata)
    }

    pub fn from_resource_path(resource_path: &String) -> E<Self> {
        let full_path = if resource_path.starts_with('/') {
            resource_path.clone()
        } else {
            format!(
                "{}/{}",
                std::env::var("ASSETS_DIR").unwrap_or("../assets".to_string()),
                resource_path
            )
        };
        let metadata_path = format!("{}/metadata.json", full_path);
        log::debug!("Path is {}", metadata_path);
        let mut metadata = Metadata::from_filename(metadata_path)?;
        metadata.enclosing_directory = full_path;
        Ok(metadata)
    }
}
