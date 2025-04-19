#!/usr/bin/env bash

source .env

client_id=$SPOTIFY_CLIENT_ID
client_secret=$SPOTIFY_CLIENT_SECRET

redirect_uri=http://localhost:8082
auth_endpoint="https://accounts.spotify.com/authorize/?response_type=code&client_id=$client_id&redirect_uri=$redirect_uri&scope=playlist-modify-public"
echo "Go here in browser:"
echo "$auth_endpoint"

read -rp "Enter the code in the URL: " code

token=$(curl https://accounts.spotify.com/api/token \
  -H "Content-Type:application/x-www-form-urlencoded" \
  -d "grant_type=authorization_code&code=$code&redirect_uri=$redirect_uri&client_id=$client_id&client_secret=$client_secret" | jq -r .refresh_token)

echo "SPOTIFY_REFRESH_TOKEN=$token"
