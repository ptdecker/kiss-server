# Phase 8: AWS Infrastructure - Context

**Gathered:** 2026-03-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Provision the AWS infrastructure needed to host kiss-server: an EC2 t3.micro instance (Amazon Linux 2023, x86_64), an Elastic IP associated with it, and a Security Group with the correct inbound rules. No Rust code changes. No service setup — that is Phase 9.

EC2 service setup (Phase 9), DNS configuration (Phase 10), and CD pipeline (Phase 11) are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Provisioning approach
- Create `scripts/setup-aws-infra.sh` following the established `scripts/` convention from Phases 6 & 7
- Script is idempotent — safe to re-run (creates resources only if they don't exist)
- Setup-only: no teardown flags; teardown is a rare manual operation
- Executor runs the script live against the real AWS account during the plan
- Region: `us-east-1` (N. Virginia)

### AWS CLI setup
- Install AWS CLI locally as a task within this phase (`brew install awscli`)
- Configure credentials/profile (AWS access key + region) before running the setup script
- Phase plan includes a task to verify `aws sts get-caller-identity` succeeds before provisioning

### SSH Security Group rule
- Port 22: open to `0.0.0.0/0` — key auth is the only gate; no IP restriction
  - Rationale: developer works from two locations (home + work), and Phase 11 CD pipeline SSHs from GitHub Actions; managing CIDRs for all three is more friction than security benefit for this project
- Port 80: open to `0.0.0.0/0` (required for web serving)
- All other inbound: deny

### Key pair
- Import `~/.ssh/id_ed25519.pub` into AWS as a new key pair named `kiss-server`
- Setup script handles the import via `aws ec2 import-key-pair`
- No new private key to manage — developer SSHs with their existing key

### Instance configuration
- Type: t3.micro (locked from roadmap — matches CI runner architecture)
- AMI: Amazon Linux 2023, x86_64 (latest at time of execution)
- Region: us-east-1

### Claude's Discretion
- Security Group and key pair naming convention (beyond `kiss-server` for the key)
- Exact AMI lookup method (aws ec2 describe-images with owner/name filters)
- VPC and subnet selection (default VPC is fine)
- Instance naming/tagging convention

</decisions>

<specifics>
## Specific Ideas

- Phase 12 docs/ci-cd.md will document the devops guide. The Phase 8 SUMMARY.md should include an **Infrastructure Notes** section capturing: key pair name, Elastic IP address, Security Group name/ID, instance ID, and region — so Phase 12 has everything it needs to write the guide without re-querying AWS.
- The `scripts/setup-aws-infra.sh` script should print the allocated Elastic IP at the end so it can be recorded and used in Phase 10 (DNS) and Phase 11 (CD).

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `scripts/ci.sh`: Established the `scripts/` pattern — bash scripts with `set -euo pipefail`, short and focused
- `scripts/setup-branch-protection.sh`: Second script in the pattern — uses CLI tools (`gh`), idempotent check-then-create logic

### Established Patterns
- `scripts/` directory: automation scripts that are committed to the repo and run once during phase execution
- Idempotent scripts: check-if-exists before creating (prevents duplicate resources on re-run)

### Integration Points
- Elastic IP from this phase → used in Phase 10 (GoDaddy DNS A record) and Phase 11 (CD deploy target)
- Key pair `kiss-server` → used in Phase 9 (SSH into instance to install service) and Phase 11 (CD pipeline SSH access)
- Security Group → referenced in Phase 9 when connecting to install kiss-server

</code_context>

<deferred>
## Deferred Ideas

- **GitHub Actions IP allowlist for SSH**: Eventually restrict port 22 to the GitHub Actions CIDR ranges instead of 0.0.0.0/0. Deferred because GitHub's IP ranges change and require maintenance. Track as a backlog item in the GitHub project.

</deferred>

---

*Phase: 08-aws-infrastructure*
*Context gathered: 2026-03-11*
