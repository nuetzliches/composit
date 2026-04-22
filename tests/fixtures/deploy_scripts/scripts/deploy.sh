#!/usr/bin/env bash
set -euo pipefail

ENV=${1:-staging}
echo "Deploying to $ENV..."
docker build -t myapp:latest .
docker push myapp:latest
kubectl apply -f k8s/
