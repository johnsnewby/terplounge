use crate::error::{Er, E};
use crate::metadata::Metadata;
use crate::session::find_session_with_uuid;
use askama::Template; // bring trait in scope
use serde::Serialize;
use serde_json::json;
use similar::{ChangeTag, DiffTag, TextDiff};
use std::fs;

fn escape(from: String) -> String {
    from.replace('\'', "\\'")
        .replace('\n', "\\\n")
        .replace('\"', "\\\"")
}

#[derive(Template)]
#[template(path = "compare.html", escape = "none")]
pub struct Comparison {
    resource: String,
    uuid: String,
    lang: String,
}

#[derive(Template)]
#[template(path = "practice.html", escape = "none")]
pub struct PracticeData {
    metadata: Metadata,
    resource_path: String,
    lang: String,
}

fn get_translation(resource_path: &String, lang: &String) -> E<String> {
    let metadata = Metadata::from_resource_path(resource_path)?;
    let source_path = format!(
        "{}/{}",
        metadata.enclosing_directory,
        metadata.translations.get(lang).unwrap()
    );
    let source = fs::read_to_string(source_path.clone())?;
    Ok(source)
}

async fn get_comparison(resource_path: &String, uuid: &String, lang: &String) -> E<Comparison> {
    Ok(Comparison {
        resource: resource_path.clone(),
        uuid: uuid.clone(),
        lang: lang.clone(),
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

    let comparison = get_comparison(&resource_path, &uuid, &lang).await?;
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
    log::debug!("Changes: {}", json!(changes).to_string());
    Ok(changes)
}

pub async fn compare(
    resource_path: String,
    uuid: String,
    lang: String,
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let template = match get_comparison(&resource_path, &uuid, &lang).await {
        Ok(c) => c,
        Err(e) => {
            log::error!("Couldn't get transcript for uuid {}: {:?}", uuid, e);
            return Err(warp::reject::reject());
        }
    };
    Ok(warp::reply::html(template.render().unwrap()))
}

pub async fn practice(
    resource_path: String,
    lang: String,
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let metadata = match Metadata::from_resource_path(&resource_path) {
        Ok(m) => m,
        Err(e) => {
            log::error!("Error: {:?}", e);
            return Err(warp::reject::not_found());
        }
    };
    let template = PracticeData {
        metadata,
        resource_path,
        lang,
    };

    Ok(warp::reply::html(template.render().unwrap()))
}
