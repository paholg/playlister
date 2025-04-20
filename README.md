# r/listentothis playlist updater

This is a tool to scrape hot of https://reddit.com/r/listentothis and update
playlists with the contents.

Spotify playlist:
https://open.spotify.com/playlist/0QLH8AqDfjGmcWK1vnf2sI

Tidal playlist:
https://listen.tidal.com/playlist/7772be23-3b43-418b-b403-2b4832f8a76f

Expect the playlists to contain roughly 70 of the top 100 posts on
r/listentothis. I run it every hour.

If you have any suggestions for improvement, or requests, please file an issue
or open a PR!

## Contributing

To run locally, you will need access to both Reddit and your music app of
choice. You will need the following variables set, which you can do with a .env
file:

```
CACHE_DIR # Optional, will cache search results if set.
LOG_LEVEL # Optional, defaults to info

REDDIT__CLIENT_ID
REDDIT__CLIENT_SECRET

# For Spotify:
SPOTIFY__CLIENT_ID
SPOTIFY__CLIENT_SECRET
SPOTIFY__REFRESH_TOKEN
SPOTIFY__PLAYLIST_ID

# For Tidal:
TIDAL__CLIENT_ID
TIDAL__CLIENT_SECRET
TIDAL__REFRESH_TOKEN
TIDAL__PLAYLIST_ID
```

The `_ID`s and `_SECRET`s for reddit and spotify come from setting up a
developer application on the respective sites.

The `SPOTIFY_REFRESH_TOKEN` needs to be for a user account, and can be generated
using the `spotify_token.sh` script.

The Tidal information can be obtained from the `tidal_token.py` script.

Feel free to reach out to me if you need any help.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
