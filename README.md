# r/listentothis Spotify playlist updater

This is just a simple tool to scrape hot of r/listentothis and to update a Spotify playlist with the
contents.

You can expect the playlist to be updated roughly once per day, and can find it here:
https://open.spotify.com/playlist/0QLH8AqDfjGmcWK1vnf2sI

It does a pretty rough job searching Spotify, so expect it to miss tracks. Expect the playlist to
contain roughly 70 of the top 100 posts on r/listentothis.

If you have any suggestions for improvement, or requests, please file an issue or open a PR!

## Contributing

To run locally, you will need access to both Reddit and Spotify. You will want a `.env` file with
the following environment variables:

```
REDDIT_CLIENT_ID
REDDIT_CLIENT_SECRET
REDDIT_USERNAME
REDDIT_PASSWORD

SPOTIFY_CLIENT_ID
SPOTIFY_CLIENT_SECRET
SPOTIFY_PLAYLIST_ID
SPOTIFY_REFRESH_TOKEN
```

The `_ID`s and `_SECRET`s come from setting up a developer application on the respective sites. The
`SPOTIFY_REFRESH_TOKEN` needs to be for a user account, and can be generated using the
`token_walkthrough.sh` script.

Feel free to reach out to me if you need any help.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
