# Phase 9: EC2 Service Setup - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-24

---

## Area 1: Binary Deployment Method

**Q:** How does the kiss-server binary get onto the EC2 instance in Phase 9?

| Option | Description |
|--------|-------------|
| Cargo build on EC2 ✓ | SSH in, clone repo, cargo build --release |
| SCP from local | Build locally, SCP binary |
| Download from GitHub Releases | Requires CD pipeline (circular dep with Phase 11) |

**Selected:** Cargo build on EC2

**Follow-up Q:** Clone from GitHub or SCP source?

| Option | Description |
|--------|-------------|
| Clone from GitHub ✓ | Self-contained: install rustup, clone, build |
| SCP source from local | Ties to local working tree |

**Selected:** Clone from GitHub

**Notes from user:** Create a GitHub issue to switch to downloading from GitHub Releases once CD pipeline (Phase 11) is in place.

---

## Area 2: Hello World Content

**Q:** What should the Hello World index.html contain?

| Option | Description |
|--------|-------------|
| Minimal placeholder ✓ | Bare-bones valid HTML — deployment smoke test |
| Project-branded page | ptodd.org branding, GitHub link |
| User-specified content | Custom copy/markup |

**Selected:** Minimal placeholder

---

## Area 3: Script Structure

**Q:** How should the Phase 9 work be scripted?

| Option | Description |
|--------|-------------|
| One setup-ec2-service.sh | Single idempotent script (consistent with Phase 8) |
| Separate scripts per concern ✓ | More granular, independently re-runnable |

**Selected:** Separate scripts

**Follow-up Q:** Naming/grouping?

| Option | Description |
|--------|-------------|
| Three scripts ✓ | install-kiss-server.sh, setup-webroot.sh, setup-iptables.sh |
| Two scripts | Combine iptables with service |
| Claude decides | Claude picks split |

**Selected:** Three scripts — `install-kiss-server.sh`, `setup-webroot.sh`, `setup-iptables.sh`

---

## Area 4: iptables Persistence

**Q:** How should port 80 → 8080 redirect survive reboots on Amazon Linux 2023?

| Option | Description |
|--------|-------------|
| iptables-services ✓ | dnf install, save rules, restore on boot |
| systemd ExecStartPost | Rule in service unit — doesn't survive service stop |
| nftables | Modern AL2023 alternative, different syntax |

**Selected:** iptables-services package

---

*Discussion completed: 2026-03-24*
