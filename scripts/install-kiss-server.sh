#!/usr/bin/env bash
set -euo pipefail

# install-kiss-server.sh
# Idempotent script to install kiss-server on an Amazon Linux 2023 EC2 instance.
# Run as ec2-user via SSH from the deployment machine.
#
# Usage: bash scripts/install-kiss-server.sh
# Requirements: Must be run on the EC2 instance (not the local machine)
#
# Steps:
#   1.   Create swap file (OOM guard on t3.micro)
#   2.   Install git
#   2.5. Install gcc (C linker required by cargo)
#   3.   Install rustup / cargo (stable toolchain)
#   4.   Clone or update repo at /opt/ptodd
#   5.   Build release binary
#   6.   Install binary as /usr/local/bin/kiss-server
#   7.   Create kiss-server system user
#   8.   Write and enable systemd unit
#   9.   Enable and start the service

CLONE_DIR="/opt/ptodd"
REPO_URL="https://github.com/ptdecker/kiss-server.git"
REPO_BRANCH="main"
BINARY_SRC="$CLONE_DIR/target/release/ptodd"
BINARY_DEST="/usr/local/bin/kiss-server"
KISS_USER="kiss-server"
SERVICE_FILE="/etc/systemd/system/kiss-server.service"
WEBROOT="/var/www/ptodd.org"

# ─── Step 1: Swap file (OOM guard on t3.micro) ────────────────────────────────

echo "==> Step 1: Swap file"

if swapon --show | grep -q /swapfile; then
  echo "  Swap file already active, skipping."
else
  echo "  Creating 512MB swap file..."
  sudo dd if=/dev/zero of=/swapfile bs=1M count=512 status=progress
  sudo chmod 600 /swapfile
  sudo mkswap /swapfile
  sudo swapon /swapfile
  echo "  Swap active."
fi

# ─── Step 2: Install git ──────────────────────────────────────────────────────

echo "==> Step 2: git"

if command -v git &>/dev/null; then
  echo "  git already installed, skipping."
else
  echo "  Installing git..."
  sudo dnf install -y git
fi

# ─── Step 2.5: Install gcc (C linker required by cargo) ──────────────────────

echo "==> Step 2.5: gcc (C linker)"

if command -v gcc &>/dev/null; then
  echo "  gcc already installed, skipping."
else
  echo "  Installing gcc..."
  sudo dnf install -y gcc
fi

# ─── Step 3: Install rustup / cargo ──────────────────────────────────────────

echo "==> Step 3: Rust toolchain"

if command -v cargo &>/dev/null; then
  echo "  Rust toolchain already installed, skipping."
else
  echo "  Installing rustup (stable)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
fi

# shellcheck source=/dev/null
source "$HOME/.cargo/env"

# ─── Step 4: Clone or update repo ────────────────────────────────────────────

echo "==> Step 4: Repo at $CLONE_DIR"

if [ -d "$CLONE_DIR/.git" ]; then
  echo "  Repo already cloned, pulling latest ($REPO_BRANCH)..."
  git -C "$CLONE_DIR" fetch origin
  git -C "$CLONE_DIR" checkout "$REPO_BRANCH"
  git -C "$CLONE_DIR" pull origin "$REPO_BRANCH"
else
  echo "  Cloning repo ($REPO_BRANCH) to $CLONE_DIR..."
  sudo git clone --branch "$REPO_BRANCH" "$REPO_URL" "$CLONE_DIR"
  sudo chown -R ec2-user:ec2-user "$CLONE_DIR"
fi

# ─── Step 5: Build release binary ─────────────────────────────────────────────

echo "==> Step 5: cargo build --release"
echo "  Building release binary (this may take a few minutes)..."
cargo build --release --manifest-path "$CLONE_DIR/Cargo.toml"

# ─── Step 6: Install binary (rename ptodd → kiss-server) ─────────────────────
# Stop service before copying — cannot overwrite a running binary (Text file busy).

echo "==> Step 6: Install binary"
echo "  Stopping kiss-server (if running) before binary install..."
sudo systemctl stop kiss-server 2>/dev/null || true
echo "  Installing binary to $BINARY_DEST..."
sudo cp "$BINARY_SRC" "$BINARY_DEST"
sudo chmod +x "$BINARY_DEST"

# ─── Step 7: Create kiss-server service user ──────────────────────────────────

echo "==> Step 7: Service user"

if id "$KISS_USER" &>/dev/null; then
  echo "  User '$KISS_USER' already exists, skipping."
else
  echo "  Creating system user '$KISS_USER'..."
  sudo useradd --system --no-create-home --shell /sbin/nologin "$KISS_USER"
fi

# ─── Step 8: Write systemd unit ───────────────────────────────────────────────

echo "==> Step 8: Systemd unit"

sudo tee "$SERVICE_FILE" > /dev/null << 'UNIT'
[Unit]
Description=kiss-server static file server
After=network.target

[Service]
Type=simple
User=kiss-server
ExecStart=/usr/local/bin/kiss-server --root /var/www/ptodd.org --port 8080
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
UNIT

echo "  Systemd unit written to $SERVICE_FILE"
sudo systemctl daemon-reload

# ─── Step 9: Enable and start service ─────────────────────────────────────────

echo "==> Step 9: Enable and start kiss-server"
sudo systemctl enable kiss-server
sudo systemctl restart kiss-server
echo "  kiss-server enabled and started."

echo ""
echo "install-kiss-server.sh complete."
