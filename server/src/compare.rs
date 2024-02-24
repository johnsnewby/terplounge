use crate::error::{Er, E};
use crate::metadata::Metadata;
use crate::session::find_session_with_uuid;
use serde::Serialize;
use serde_json::json;
use similar::{ChangeTag, TextDiff};
use std::fs;

fn get_translation(resource_path: &String, lang: &String) -> E<String> {
    let metadata = Metadata::from_resource_path(resource_path)?;
    let source_path = format!(
        "{}/{}",
        metadata.enclosing_directory,
        metadata.translations.get(lang).expect(&format!(
            "Translation not found for resource {} and lang {}",
            resource_path, lang
        ))
    );
    let source = fs::read_to_string(source_path.clone())?;
    Ok(source)
}

pub struct Comparison {
    pub resource: String,
    pub uuid: String,
    pub lang: String,
}

pub async fn get_comparison(resource_path: &str, uuid: &str, lang: &str) -> E<Comparison> {
    Ok(Comparison {
        resource: resource_path.to_string(),
        uuid: uuid.to_owned(),
        lang: lang.to_string(),
    })
}

#[derive(Clone, Serialize)]
pub struct Change {
    pub change_type: String,
    pub content: String,
}

pub async fn changes(resource_path: String, uuid: String, lang: String) -> E<Vec<Change>> {
    let source = get_translation(&resource_path, &lang)?;
    let session_id = find_session_with_uuid(&uuid)
        .await
        .expect("Session not found");

    let session = match crate::session::get_session(&session_id).await {
        Some(s) => s,
        None => return Err(Er::new(format!("Session {} not found", session_id))),
    };

    let dest = session.transcript()?;

    log::debug!("Comparing");

    let diff = TextDiff::configure().diff_words(dest.as_str(), source.as_str());
    let changes: Vec<Change> = diff
        .iter_all_changes()
        .map(|x| Change {
            change_type: match x.tag() {
                ChangeTag::Equal => "equal".to_string(),
                ChangeTag::Delete => "delete".to_string(),
                ChangeTag::Insert => "insert".to_string(),
            },
            content: x.value().to_string(),
        })
        .collect();
    log::trace!("Changes: {}", json!(changes).to_string());
    Ok(changes)
}
