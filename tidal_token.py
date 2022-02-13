#!/usr/bin/env python

import tidalapi

session = tidalapi.Session()
session.login_oauth_simple()

print("client_id: ", session.config.client_id)
print("client_secret: ", session.config.client_secret)
print("refresh_token: ", session.refresh_token)
