use crate::{AuthResponse, JsonRequest, Secret, track::Track};
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

#[derive(Deserialize, Debug)]
pub struct Settings {
    client_id: String,
    client_secret: Secret<String>,
}

pub struct Reddit {
    access_token: Secret<String>,
    client: reqwest::Client,
}

impl Reddit {
    pub async fn new(config: Settings, client: reqwest::Client) -> eyre::Result<Reddit> {
        let access_token = get_access_token(&client, &config).await?;

        Ok(Reddit {
            access_token,
            client,
        })
    }

    pub async fn tracks(&self, subreddit: &str, regex: regex::Regex) -> eyre::Result<Vec<Track>> {
        let tracks: Vec<_> = self
            .posts(subreddit)
            .await?
            .map(|post| post.title)
            .filter_map(move |title| match regex.captures(&title) {
                Some(cap) => Some(Track::new(cap[1].to_string(), cap[2].to_string())),
                None => {
                    warn!("Failed to match: {}", title);
                    None
                }
            })
            .collect();
        Ok(tracks)
    }

    async fn posts(&self, subreddit: &str) -> eyre::Result<impl Iterator<Item = Post> + use<>> {
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
            .get(url(subreddit))
            .bearer_auth(self.access_token.expose_secret())
            .header(
                reqwest::header::USER_AGENT,
                format!("listothis-playlist-updater/{}", env!("CARGO_PKG_VERSION")),
            )
            .send_it_json::<Response>()
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
    format!("https://oauth.reddit.com/{}?limit=100", subreddit)
}

async fn get_access_token(
    client: &reqwest::Client,
    config: &Settings,
) -> eyre::Result<Secret<String>> {
    let response: AuthResponse = client
        .post("https://www.reddit.com/api/v1/access_token")
        .basic_auth(
            &config.client_id,
            Some(config.client_secret.expose_secret()),
        )
        .header(
            reqwest::header::USER_AGENT,
            format!("listothis-playlist-updater/{}", env!("CARGO_PKG_VERSION")),
        )
        .form(&[("grant_type", "client_credentials")])
        .send_it_json()
        .await?;

    Ok(response.access_token)
}
