#!/usr/bin/env bash
set -euo pipefail

echo "Bootstrapping environment..."
cp .env.example .env
docker compose up -d db
sleep 2
docker compose run --rm app python manage.py migrate
