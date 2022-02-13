use crate::{track::Track, AuthResponse};
use serde::Deserialize;
use std::env;
use tracing::warn;

struct Post {
    title: String,
}

impl Post {
    fn new(title: String) -> Post {
        Post { title }
    }
}

pub struct Reddit {
    access_token: String,
    client: reqwest::Client,
}

impl Reddit {
    pub async fn new(client: reqwest::Client) -> eyre::Result<Reddit> {
        let access_token = get_access_token(&client).await?;

        Ok(Reddit {
            access_token,
            client,
        })
    }

    pub async fn tracks(
        &self,
        subreddit: &str,
        regex: regex::Regex,
    ) -> eyre::Result<impl Iterator<Item = Track>> {
        let tracks = self
            .posts(subreddit)
            .await?
            .map(|post| post.title)
            .map(|title| htmlescape::decode_html(&title).unwrap_or(title))
            .filter_map(move |title| match regex.captures(&title) {
                Some(cap) => Some(Track::new(cap[1].to_string(), cap[2].to_string())),
                None => {
                    warn!("Failed to match: {}", title);
                    None
                }
            });
        Ok(tracks)
    }

    async fn posts(&self, subreddit: &str) -> eyre::Result<impl Iterator<Item = Post>> {
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
            .send()
            .await?
            .error_for_status()?
            .json::<Response>()
            .await?
            .data
            .children
            .into_iter()
            .map(|child| child.data.title)
            .map(|title| htmlescape::decode_html(&title).unwrap_or(title))
            .map(Post::new);
        Ok(posts)
    }
}

fn url(subreddit: &str) -> String {
    format!("https://oauth.reddit.com/{}", subreddit)
}

async fn get_access_token(client: &reqwest::Client) -> eyre::Result<String> {
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
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(response.access_token)
}
