#!/usr/bin/env bash
set -euo pipefail

# setup-iptables.sh
# Idempotent script to configure iptables rules on an Amazon Linux 2023 EC2 instance:
#   - PREROUTING REDIRECT: port 80 → 8080
#   - INPUT ACCEPT: port 80
#   - INPUT ACCEPT: port 8080 (redirected traffic traverses the INPUT chain)
#   - Persists rules via iptables-services
#
# Usage: bash scripts/setup-iptables.sh
# Requirements: Must be run on the EC2 instance (not the local machine)
#
# Steps:
#   1. Install iptables-services (if not present)
#   2. Start and enable iptables service
#   3. Add PREROUTING REDIRECT rule (80 → 8080)
#   4. Add INPUT ACCEPT for port 80
#   5. Add INPUT ACCEPT for port 8080
#   6. Save rules for persistence across reboots

IPTABLES_SERVICE="iptables"

# ─── Step 1: Install iptables-services ────────────────────────────────────────

echo "==> Step 1: iptables-services"

if rpm -q iptables-services &>/dev/null; then
  echo "  iptables-services already installed, skipping."
else
  echo "  Installing iptables-services..."
  sudo dnf install -y iptables-services
fi

# ─── Step 2: Start and enable iptables service ────────────────────────────────
# Must precede 'service iptables save' — the save command requires the service running.

echo "==> Step 2: Start and enable iptables service"
sudo systemctl start "$IPTABLES_SERVICE"
sudo systemctl enable "$IPTABLES_SERVICE"
echo "  iptables service started and enabled."

# ─── Step 3: PREROUTING REDIRECT rule (80 → 8080) ────────────────────────────

echo "==> Step 3: PREROUTING REDIRECT rule"

if sudo iptables -t nat -C PREROUTING -p tcp --dport 80 -j REDIRECT --to-port 8080 2>/dev/null; then
  echo "  PREROUTING REDIRECT rule already exists, skipping."
else
  echo "  Adding PREROUTING REDIRECT rule (80 -> 8080)..."
  sudo iptables -t nat -A PREROUTING -p tcp --dport 80 -j REDIRECT --to-port 8080
fi

# ─── Step 4: INPUT ACCEPT for port 80 ────────────────────────────────────────
# Use -I INPUT 4 to INSERT before position 5 (the default REJECT all rule from
# iptables-services). Using -A (append) places the rule AFTER the REJECT, making
# it unreachable.

echo "==> Step 4: INPUT ACCEPT port 80"

if sudo iptables -C INPUT -p tcp --dport 80 -j ACCEPT 2>/dev/null; then
  echo "  INPUT ACCEPT port 80 already exists, skipping."
else
  echo "  Adding INPUT ACCEPT rule for port 80 (insert at position 4)..."
  sudo iptables -I INPUT 4 -p tcp --dport 80 -j ACCEPT
fi

# ─── Step 5: INPUT ACCEPT for port 8080 ──────────────────────────────────────
# Required: redirected traffic (80 → 8080) traverses the INPUT chain, not just FORWARD.
# Insert at position 5 (after port 80 rule, before REJECT).

echo "==> Step 5: INPUT ACCEPT port 8080"

if sudo iptables -C INPUT -p tcp --dport 8080 -j ACCEPT 2>/dev/null; then
  echo "  INPUT ACCEPT port 8080 already exists, skipping."
else
  echo "  Adding INPUT ACCEPT rule for port 8080 (insert at position 5)..."
  sudo iptables -I INPUT 5 -p tcp --dport 8080 -j ACCEPT
fi

# ─── Step 6: Save rules for persistence ──────────────────────────────────────

echo "==> Step 6: Save iptables rules"
echo "  Saving iptables rules..."
sudo service iptables save
echo "  Rules saved to /etc/sysconfig/iptables"

echo ""
echo "setup-iptables.sh complete."
