#!/usr/bin/env bash
# Dependencies needed inside the claude-sandbox container for this project.
# This script runs as root on every container creation.
set -euo pipefail

apt-get update -y
apt-get install -y build-essential gcc cargo openjdk-21-jdk-headless
