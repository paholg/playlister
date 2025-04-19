#!/usr/bin/env python

import tidalapi

session = tidalapi.Session()
session.login_oauth_simple()

print("TIDAL_CLIENT_ID=", session.config.client_id)
print("TIDAL_CLIENT_SECRET=", session.config.client_secret)
print("TIDAL_REFRESH_TOKEN=", session.refresh_token)
