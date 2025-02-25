#!/usr/bin/env bash

set -ex

if ! [ -x "$(command -v sqlx)" ]; then
  echo >&2 "Error: sqlx is not installed."
  echo >&2 "Use:"
  echo >&2 "    cargo install sqlx-cli --version=0.6.2"
  echo >&2 "to install it."
  exit 1
fi

cd ./docker/
docker-compose -f docker-compose.dev.yml down
cd ..
docker-compose -f docker/docker-compose.dev.yml up -d

echo "Waiting 10 seconds for pg to come alive.."
sleep 10

sqlx migrate run
cargo run
