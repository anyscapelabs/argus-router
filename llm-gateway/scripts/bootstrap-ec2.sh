#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y curl docker.io docker-compose-v2 unzip

if ! command -v aws > /dev/null; then
  curl --fail --silent --show-error \
    https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip \
    --output /tmp/awscliv2.zip
  rm -rf /tmp/aws
  unzip -q /tmp/awscliv2.zip -d /tmp
  /tmp/aws/install
fi

systemctl enable --now docker

install -d -m 0755 -o ubuntu -g ubuntu /opt/llm-gateway
