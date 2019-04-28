use crate::{track::Track, AuthResponse};
use serde::Deserialize;
use std::env;

#[derive(Deserialize, Debug)]
struct Post {
    title: String,
}

pub struct Reddit<'a> {
    access_token: String,
    client: &'a reqwest::Client,
}

impl<'a> Reddit<'a> {
    pub fn new(client: &reqwest::Client) -> Result<Reddit, failure::Error> {
        let access_token = get_access_token(client)?;

        Ok(Reddit {
            access_token,
            client,
        })
    }

    pub fn listentothis_hot(&self) -> Result<impl Iterator<Item = Track>, failure::Error> {
        let re = regex::Regex::new("(.*) - (.*?) +[\\(\\[]")?;

        #[derive(Deserialize, Debug)]
        struct Response {
            data: ResponseData,
        }

        #[derive(Deserialize, Debug)]
        struct ResponseData {
            children: Vec<ChildData>,
        }

        #[derive(Deserialize, Debug)]
        struct ChildData {
            data: Child,
        }

        #[derive(Deserialize, Debug)]
        struct Child {
            author_flair_text: Option<String>,
            title: String,
        }

        let tracks = self
            .client
            .get("https://oauth.reddit.com/r/listentothis/hot")
            .bearer_auth(&self.access_token)
            .header(
                reqwest::header::USER_AGENT,
                format!("listothis-playlist-updater/{}", env!("CARGO_PKG_VERSION")),
            )
            .send()?
            .json::<Response>()?
            .data
            .children
            .into_iter()
            .map(|child| child.data)
            .filter_map(|child| {
                if child.author_flair_text == Some("robot".to_string()) {
                    None
                } else {
                    Some(child.title)
                }
            })
            .filter_map(move |title| {
                re.captures(&title)
                    .map(|captures| Track::new(captures[1].into(), captures[2].into()))
            });
        Ok(tracks)
    }
}

fn get_access_token(client: &reqwest::Client) -> Result<String, failure::Error> {
    let body = format!(
        "grant_type=password&username={}&password={}",
        env::var("REDDIT_USERNAME")?,
        env::var("REDDIT_PASSWORD")?
    );

    let response: AuthResponse = client
        .post("https://www.reddit.com/api/v1/access_token")
        .basic_auth(
            env::var("REDDIT_CLIENT_ID")?,
            Some(env::var("REDDIT_CLIENT_SECRET")?),
        )
        .header(
            reqwest::header::USER_AGENT,
            format!("listothis-playlist-updater/{}", env!("CARGO_PKG_VERSION")),
        )
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()?
        .json()?;

    Ok(response.access_token)
}
