use askama::Template; // bring trait in scope

use crate::compare::practice;
use crate::metadata::Metadata;
use crate::session::{get_sessions, mark_session_for_closure_uuid, user_connected, SessionData};
use crate::translate;
use bytes::Bytes;
use crossbeam_channel::Sender;
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use warp::reply::Json;
use warp::{http::Response, Filter};

#[derive(Template)]
#[template(path = "index.html", escape = "none")]
pub struct Index {
    sessions: Vec<SessionData>,
}

pub async fn index() -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let mut sessions = get_sessions().await.ok_or(warp::reject::reject())?;
    sessions.sort_by(|a, b| {
        a.created_at
            .partial_cmp(&b.created_at)
            .expect("Unexpected error in comparison")
    });

    let template = Index { sessions };

    Ok(warp::reply::html(template.render().unwrap()))
}

pub async fn serve_resource(
    resource_path: String,
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let metadata = match Metadata::from_resource_path(&resource_path) {
        Ok(m) => m,
        Err(e) => {
            log::error!("Error: {:?} loading {}", e, resource_path);
            return Err(warp::reject::not_found());
        }
    };
    let content_path = format!("{}/{}", metadata.enclosing_directory, metadata.audio);
    log::debug!("content_path is {}", content_path);
    let mut f = std::fs::File::open(content_path.clone()).unwrap();
    let metadata = std::fs::metadata(&content_path).expect("unable to read metadata");
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).expect("buffer overflow");
    let b: Bytes = Bytes::from(buffer);
    let response = match Response::builder().body(b) {
        Ok(b) => b,
        Err(e) => {
            log::error!("Error making response: {:?}", e);
            return Err(warp::reject::not_found());
        }
    };
    Ok(response)
}

pub async fn serve(translate_tx: Sender<translate::TranslationRequest>) {
    let chat = warp::path("chat")
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::ws())
        .map(move |params: HashMap<String, String>, ws: warp::ws::Ws| {
            let tx = translate_tx.clone();
            let lang: String = (params.get("lang").unwrap_or(&"de".to_string())).clone();
            let resource: Option<String> = match params.get("resource") {
                Some(s) => Some(s.clone()),
                None => None,
            };
            let sample_rate: u32 = match params.get("rate") {
                Some(rate) => rate.to_string(),
                None => "44100".to_string(),
            }
            .parse()
            .unwrap();
            ws.on_upgrade(move |socket| {
                user_connected(socket, tx.clone(), lang, sample_rate, resource)
            })
        });

    let close = warp::post().and(warp::path!("close" / String).and_then(async move |uuid| {
        mark_session_for_closure_uuid(uuid).await;
        Ok::<&str, warp::Rejection>("foo")
    }));

    let practice = warp::get().and(
        warp::path!("practice" / String / String)
            .and_then(async move |directory, lang| practice(directory, lang).await),
    );

    let serve_resource = warp::get().and(
        warp::path!("serve_resource" / String)
            .and_then(async move |resource_path| serve_resource(resource_path).await),
    );

    let status = warp::path!("status" / String).and_then(async move |uuid| {
        match crate::session::find_session_with_uuid(&uuid).await {
            Some(session_id) => match crate::session::get_session(&session_id).await {
                Some(session) => Ok::<Json, warp::Rejection>(warp::reply::json(&session)),
                None => Err(warp::reject::not_found()),
            },
            None => Err(warp::reject::not_found()),
        }
    });

    let compare = warp::get()
        .and(warp::path!("compare" / String / String / String))
        .and_then(async move |asset_id, uuid, lang| {
            match crate::compare::compare(asset_id, uuid, lang).await {
                Ok(x) => Ok(x),
                Err(e) => {
                    log::error!("Error in compare: {:?}", e);
                    Err(warp::reject())
                }
            }
        });

    let recordings_dir = std::env::var("RECORDINGS_DIR").unwrap_or("../recordings".to_string());

    let recordings = warp::get()
        .and(warp::path("recordings"))
        .and(warp::fs::dir(recordings_dir));

    let assets_dir = std::env::var("ASSETS_DIR").unwrap_or("../assets".to_string());
    let assets = warp::get()
        .and(warp::path("assets"))
        .and(warp::fs::dir(assets_dir));

    let transcript = warp::path!("transcript" / String).and_then(async move |uuid| {
        match crate::session::find_session_with_uuid(&uuid).await {
            Some(session_id) => match crate::session::get_session(&session_id).await {
                Some(session) => Ok(session.transcript().unwrap()),
                None => Err(warp::reject::not_found()),
            },
            None => Err(warp::reject::not_found()),
        }
    });

    let index = warp::path::end().and_then(async move || crate::api::index().await);

    #[derive(RustEmbed)]
    #[folder = "../client"]
    struct StaticContent;
    let static_content_serve = warp_embed::embed(&StaticContent);

    let routes = index
        .or(assets)
        .or(chat)
        .or(close)
        .or(compare)
        .or(practice)
        .or(recordings)
        .or(serve_resource)
        .or(status)
        .or(static_content_serve)
        .or(transcript);
    log::debug!("Starting server");
    let listen;
    if let Ok(x) = std::env::var(" LISTEN") {
        listen = x.parse().unwrap();
    } else {
        listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3030);
    };

    warp::serve(routes).run(listen).await;
}
