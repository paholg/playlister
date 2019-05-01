use crate::{track::Track, AuthResponse};
use log::warn;
use serde::Deserialize;
use std::env;

struct Post {
    title: String,
}

impl Post {
    fn new(title: String) -> Post {
        Post { title }
    }
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

    pub fn tracks(
        &self,
        subreddit: &str,
        regex: regex::Regex,
    ) -> Result<impl Iterator<Item = Track>, failure::Error> {
        let tracks = self
            .posts(subreddit)?
            .map(|post| post.title)
            .map(|title| htmlescape::decode_html(&title).unwrap_or_else(|_| title))
            .filter_map(move |title| match regex.captures(&title) {
                Some(cap) => Some(Track::new(cap[1].to_string(), cap[2].to_string())),
                None => {
                    warn!("Failed to match: {}", title);
                    None
                }
            });
        Ok(tracks)
    }

    fn posts(&self, subreddit: &str) -> Result<impl Iterator<Item = Post>, failure::Error> {
        #[derive(Deserialize, Debug)]
        struct Response {
            data: ResponseData,
        }

        #[derive(Deserialize, Debug)]
        struct ResponseData {
            children: Vec<Child>,
        }

        #[derive(Deserialize, Debug)]
        struct Child {
            data: ChildData,
        }

        #[derive(Deserialize, Debug)]
        struct ChildData {
            title: String,
        }

        let posts = self
            .client
            .get(&url(subreddit))
            .bearer_auth(&self.access_token)
            .header(
                reqwest::header::USER_AGENT,
                format!("listothis-playlist-updater/{}", env!("CARGO_PKG_VERSION")),
            )
            .send()?
            .error_for_status()?
            .json::<Response>()?
            .data
            .children
            .into_iter()
            .map(|child| child.data.title)
            .map(|title| htmlescape::decode_html(&title).unwrap_or_else(|_| title))
            .map(|title| Post::new(title));
        Ok(posts)
    }
}

fn url(subreddit: &str) -> String {
    format!("https://oauth.reddit.com/{}", subreddit)
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
        .error_for_status()?
        .json()?;

    Ok(response.access_token)
}
