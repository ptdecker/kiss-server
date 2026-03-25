# Phase 8: AWS Infrastructure - Research

**Researched:** 2026-03-11
**Domain:** AWS CLI — EC2 provisioning, Elastic IP, Security Groups, key pair import
**Confidence:** HIGH (all commands verified against current official AWS CLI docs)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Create `scripts/setup-aws-infra.sh` following the established `scripts/` convention from Phases 6 & 7
- Script is idempotent — safe to re-run (creates resources only if they don't exist)
- Setup-only: no teardown flags; teardown is a rare manual operation
- Executor runs the script live against the real AWS account during the plan
- Region: `us-east-1` (N. Virginia)
- Install AWS CLI locally as a task within this phase (`brew install awscli`)
- Configure credentials/profile (AWS access key + region) before running the setup script
- Phase plan includes a task to verify `aws sts get-caller-identity` succeeds before provisioning
- Port 22: open to `0.0.0.0/0` — key auth is the only gate; no IP restriction
- Port 80: open to `0.0.0.0/0`
- All other inbound: deny
- Import `~/.ssh/id_ed25519.pub` into AWS as a new key pair named `kiss-server`
- Setup script handles the import via `aws ec2 import-key-pair`
- Type: t3.micro (locked from roadmap — matches CI runner architecture)
- AMI: Amazon Linux 2023, x86_64 (latest at time of execution)
- Region: us-east-1

### Claude's Discretion
- Security Group and key pair naming convention (beyond `kiss-server` for the key)
- Exact AMI lookup method (aws ec2 describe-images with owner/name filters)
- VPC and subnet selection (default VPC is fine)
- Instance naming/tagging convention

### Deferred Ideas (OUT OF SCOPE)
- **GitHub Actions IP allowlist for SSH**: Eventually restrict port 22 to the GitHub Actions CIDR ranges instead of 0.0.0.0/0. Deferred because GitHub's IP ranges change and require maintenance. Track as a backlog item in the GitHub project.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INFRA-01 | An EC2 t3.micro instance (Amazon Linux 2023, x86_64) exists and is accessible via SSH | AMI lookup via SSM parameter, `run-instances` syntax, `wait instance-running`, SSH test command |
| INFRA-02 | An Elastic IP is allocated and associated with the EC2 instance (stable, survives stop/start) | `allocate-address` + `associate-address` flow, idempotent check via `describe-addresses` |
| INFRA-03 | Security Group allows port 80 inbound from `0.0.0.0/0` and port 22 inbound from authorized IPs only | `create-security-group` + `authorize-security-group-ingress` with duplicate rule handling |
</phase_requirements>

---

## Summary

Phase 8 provisions AWS infrastructure using the AWS CLI v2. All resources are created with a single idempotent bash script (`scripts/setup-aws-infra.sh`) that follows the established project pattern: `set -euo pipefail`, check-then-create logic, and short focused sections. The script must be safe to re-run — every create operation is guarded by a describe query that exits early if the resource already exists.

The five provisioning operations in order: (1) import SSH key pair, (2) create Security Group and authorize inbound rules, (3) look up the latest Amazon Linux 2023 AMI via SSM parameter, (4) launch the EC2 instance with the key and SG, and (5) allocate and associate an Elastic IP. The script prints the Elastic IP at the end for use in Phases 10 and 11.

**Primary recommendation:** Use SSM parameter `/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64` to resolve the latest AL2023 AMI ID at runtime — this avoids hardcoding a stale AMI ID and is the AWS-recommended approach.

---

## Standard Stack

### Core
| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| AWS CLI v2 | `brew install awscli` (latest v2) | All EC2/EIP/SG provisioning | Official AWS CLI; v2 required for SSM parameter resolution in run-instances |
| bash | System bash | Script runtime | Established project convention (ci.sh, setup-branch-protection.sh) |
| python3 | System (macOS) | JSON parsing in scripts | Already used in setup-branch-protection.sh for JSON parsing |

### Supporting
| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| `aws ssm get-parameter` | included in awscli | Resolve latest AL2023 AMI ID | AMI lookup step in setup script |
| `aws sts get-caller-identity` | included in awscli | Verify credentials before provisioning | Pre-flight check task |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| AWS CLI bash script | Terraform/CDK | IaC tools are heavier; locked decision is CLI script |
| SSM AMI lookup | `aws ec2 describe-images --owners amazon` | describe-images requires sort by creation date; SSM is simpler and always current |

**Installation:**
```bash
brew install awscli
aws configure  # or: aws configure --profile kiss-server
aws sts get-caller-identity  # verify before proceeding
```

---

## Architecture Patterns

### Recommended Script Structure
```
scripts/
└── setup-aws-infra.sh   # idempotent, check-then-create for all 5 resources
```

The script follows the established project pattern:
- `#!/usr/bin/env bash` shebang
- `set -euo pipefail` at top
- Named constants (REGION, KEY_NAME, SG_NAME, INSTANCE_NAME, TAG)
- Sections: key pair, security group, AMI lookup, EC2 instance, Elastic IP
- Final `echo` printing the Elastic IP

### Pattern 1: Check-Then-Create (idempotency)

**What:** Describe the resource before creating it; skip creation if it already exists.
**When to use:** Every create operation in the script.

```bash
# Key pair
if aws ec2 describe-key-pairs --key-names "$KEY_NAME" --region "$REGION" \
    --query 'KeyPairs[0].KeyName' --output text 2>/dev/null | grep -q "$KEY_NAME"; then
  echo "Key pair '$KEY_NAME' already exists, skipping."
else
  aws ec2 import-key-pair \
    --key-name "$KEY_NAME" \
    --public-key-material fileb://~/.ssh/id_ed25519.pub \
    --region "$REGION"
fi

# Security Group
SG_ID=$(aws ec2 describe-security-groups \
  --filters "Name=group-name,Values=$SG_NAME" \
  --query 'SecurityGroups[0].GroupId' \
  --output text --region "$REGION" 2>/dev/null || echo "None")

if [ "$SG_ID" = "None" ] || [ -z "$SG_ID" ]; then
  SG_ID=$(aws ec2 create-security-group \
    --group-name "$SG_NAME" \
    --description "kiss-server security group" \
    --region "$REGION" \
    --query 'GroupId' --output text)
  # authorize-security-group-ingress only runs when SG was just created
  aws ec2 authorize-security-group-ingress \
    --group-id "$SG_ID" --protocol tcp --port 22 --cidr 0.0.0.0/0 --region "$REGION"
  aws ec2 authorize-security-group-ingress \
    --group-id "$SG_ID" --protocol tcp --port 80 --cidr 0.0.0.0/0 --region "$REGION"
fi
```

### Pattern 2: SSM Parameter for Latest AMI

**What:** Resolve the current AL2023 AMI ID at runtime via SSM rather than hardcoding.
**When to use:** AMI lookup step; ensures the script always uses the latest security-patched AMI.

```bash
# Source: https://docs.aws.amazon.com/linux/al2023/ug/ec2.html
AMI_ID=$(aws ssm get-parameter \
  --name /aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64 \
  --region "$REGION" \
  --query 'Parameter.Value' --output text)
```

### Pattern 3: run-instances + wait + tag

**What:** Launch instance, wait for running state, tag it.
**When to use:** Instance creation step.

```bash
# Source: https://docs.aws.amazon.com/cli/latest/reference/ec2/run-instances.html
INSTANCE_ID=$(aws ec2 run-instances \
  --image-id "$AMI_ID" \
  --instance-type t3.micro \
  --key-name "$KEY_NAME" \
  --security-group-ids "$SG_ID" \
  --count 1 \
  --region "$REGION" \
  --query 'Instances[0].InstanceId' --output text)

aws ec2 wait instance-running --instance-ids "$INSTANCE_ID" --region "$REGION"

aws ec2 create-tags \
  --resources "$INSTANCE_ID" \
  --tags Key=Name,Value="$INSTANCE_NAME" \
  --region "$REGION"
```

### Pattern 4: Elastic IP allocate-and-associate

**What:** Allocate an EIP (if none tagged for this project), then associate with instance.
**When to use:** Final step of the setup script.

```bash
# Check for existing EIP tagged for this project
ALLOC_ID=$(aws ec2 describe-addresses \
  --filters "Name=tag:Name,Values=$INSTANCE_NAME" \
  --query 'Addresses[0].AllocationId' \
  --output text --region "$REGION" 2>/dev/null || echo "None")

if [ "$ALLOC_ID" = "None" ] || [ -z "$ALLOC_ID" ]; then
  ALLOC_ID=$(aws ec2 allocate-address \
    --domain vpc \
    --region "$REGION" \
    --query 'AllocationId' --output text)
  aws ec2 create-tags \
    --resources "$ALLOC_ID" \
    --tags Key=Name,Value="$INSTANCE_NAME" \
    --region "$REGION"
fi

aws ec2 associate-address \
  --instance-id "$INSTANCE_ID" \
  --allocation-id "$ALLOC_ID" \
  --region "$REGION"

ELASTIC_IP=$(aws ec2 describe-addresses \
  --allocation-ids "$ALLOC_ID" \
  --query 'Addresses[0].PublicIp' --output text --region "$REGION")

echo "Elastic IP: $ELASTIC_IP"
```

### Anti-Patterns to Avoid
- **Hardcoding AMI ID:** AL2023 AMIs are updated frequently with security patches; a hardcoded ID becomes stale and may be deprecated.
- **Running authorize-security-group-ingress on re-run without checking:** Returns `InvalidPermission.Duplicate` error and fails the script (which has `set -e`). Guard it inside the SG-creation block.
- **SCP over a running binary during deploy (Phase 9+):** SIGBUS risk. Always SCP to `/tmp/` then atomic `mv` (noted in project decisions).
- **Omitting `--region` flag:** Relies on environment; explicit region makes the script portable and auditable.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Wait for instance to be reachable | Custom polling loop with sleep | `aws ec2 wait instance-running` | Built-in 15s polling, 40 retries, proper exit codes |
| Latest AMI ID | Hardcoded AMI or custom describe-images sort | SSM parameter `/aws/service/ami-amazon-linux-latest/...` | AWS-maintained, always current, single query |
| JSON parsing in bash | Custom string manipulation | `--query` (JMESPath) + `--output text` | Safer, handles AWS response schema changes |

**Key insight:** The AWS CLI's `--query` JMESPath + `--output text` combo eliminates the need for `jq` or `python3` for simple field extraction. Use it consistently.

---

## Common Pitfalls

### Pitfall 1: authorize-security-group-ingress Duplicate Rule Error
**What goes wrong:** Re-running the script after SG exists but before rules were added causes `InvalidPermission.Duplicate` which terminates the script under `set -e`.
**Why it happens:** `authorize-security-group-ingress` is not idempotent — the AWS API returns an error if the exact rule already exists.
**How to avoid:** Only call `authorize-security-group-ingress` inside the block that creates the SG (when `$SG_ID` was just created). On re-run, the SG exists and rules already exist; the block is skipped entirely.
**Warning signs:** Script fails on second run with `An error occurred (InvalidPermission.Duplicate)`.

### Pitfall 2: import-key-pair Duplicate Name Error
**What goes wrong:** Re-running the script when key pair named `kiss-server` already exists returns an error and fails the script.
**Why it happens:** `import-key-pair` is not idempotent.
**How to avoid:** Use `describe-key-pairs --key-names kiss-server` first; skip import if it succeeds (exit 0 with results).
**Warning signs:** `An error occurred (InvalidKeyPair.Duplicate) ... The keypair already exists`.

### Pitfall 3: associate-address on Already-Associated EIP
**What goes wrong:** If the EIP is already associated with the instance, `associate-address` may succeed (it allows reassociation) but generates noise or can fail in edge cases.
**Why it happens:** `associate-address` is documented as idempotent for same instance, but check first to be clean.
**How to avoid:** Tag the EIP with the project name at allocation time; check `describe-addresses --filters Name=tag:Name,Values=...` on re-run to retrieve the existing `AllocationId` without allocating a new one.
**Warning signs:** Multiple EIPs accumulating in the account (EIPs cost money when unassociated).

### Pitfall 4: instance-running Does Not Mean SSH-Ready
**What goes wrong:** `aws ec2 wait instance-running` completes, but SSH immediately after times out.
**Why it happens:** `instance-running` is the EC2 state, not sshd availability. The instance still needs to pass status checks and start sshd (~30-60 seconds after `running`).
**How to avoid:** After `wait instance-running`, add a brief SSH retry loop in the verification step (not in the setup script itself — the executor verifies manually or via a test task).
**Warning signs:** `ssh: connect to host ... port 22: Connection timed out` immediately after the script completes.

### Pitfall 5: Default VPC May Not Exist in New Accounts
**What goes wrong:** `run-instances` without `--subnet-id` assumes the default VPC exists; some AWS accounts have had it deleted.
**Why it happens:** AWS creates a default VPC per region, but it can be deleted by account administrators.
**How to avoid:** Verify default VPC exists with `aws ec2 describe-vpcs --filters Name=isDefault,Values=true`. For this project, default VPC is fine per the locked decision; just confirm it's there.
**Warning signs:** `run-instances` fails with VPC/subnet error despite no explicit subnet specified.

### Pitfall 6: t3.micro Unlimited Credit Mode Default
**What goes wrong:** t3 instances launch with `unlimited` credit mode by default (unlike t2). This means CPU bursting charges can accumulate on a lightly used instance.
**Why it happens:** AWS changed the default for t3 to `unlimited` to encourage performance.
**How to avoid:** Launch with `--credit-specification CpuCredits=standard` to cap burst charges. For a personal learning project this may not matter, but it's good practice.
**Warning signs:** Unexpected `CPUCreditUsage` charges in AWS Cost Explorer.

---

## Code Examples

### Full idempotency guard: key pair
```bash
# Source: https://docs.aws.amazon.com/cli/latest/reference/ec2/import-key-pair.html
KEY_NAME="kiss-server"
REGION="us-east-1"

EXISTING_KEY=$(aws ec2 describe-key-pairs \
  --key-names "$KEY_NAME" \
  --region "$REGION" \
  --query 'KeyPairs[0].KeyName' \
  --output text 2>/dev/null || echo "")

if [ "$EXISTING_KEY" = "$KEY_NAME" ]; then
  echo "Key pair '$KEY_NAME' already exists, skipping import."
else
  echo "Importing key pair '$KEY_NAME'..."
  aws ec2 import-key-pair \
    --key-name "$KEY_NAME" \
    --public-key-material fileb://~/.ssh/id_ed25519.pub \
    --region "$REGION"
fi
```

### AMI lookup via SSM
```bash
# Source: https://docs.aws.amazon.com/linux/al2023/ug/ec2.html
AMI_ID=$(aws ssm get-parameter \
  --name /aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64 \
  --region "$REGION" \
  --query 'Parameter.Value' \
  --output text)
echo "Using AMI: $AMI_ID"
```

### Instance existence check before run-instances
```bash
INSTANCE_ID=$(aws ec2 describe-instances \
  --filters \
    "Name=tag:Name,Values=kiss-server" \
    "Name=instance-state-name,Values=running,stopped,pending" \
  --query 'Reservations[0].Instances[0].InstanceId' \
  --output text \
  --region "$REGION" 2>/dev/null || echo "")

if [ -n "$INSTANCE_ID" ] && [ "$INSTANCE_ID" != "None" ]; then
  echo "Instance '$INSTANCE_ID' already exists, skipping launch."
else
  INSTANCE_ID=$(aws ec2 run-instances \
    --image-id "$AMI_ID" \
    --instance-type t3.micro \
    --key-name "$KEY_NAME" \
    --security-group-ids "$SG_ID" \
    --count 1 \
    --credit-specification CpuCredits=standard \
    --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=kiss-server}]" \
    --region "$REGION" \
    --query 'Instances[0].InstanceId' \
    --output text)
  echo "Launched instance: $INSTANCE_ID"
  aws ec2 wait instance-running --instance-ids "$INSTANCE_ID" --region "$REGION"
  echo "Instance is running."
fi
```

### SSH reachability test (verification task, not setup script)
```bash
# Run after setup-aws-infra.sh completes
ssh -o StrictHostKeyChecking=no \
    -o ConnectTimeout=10 \
    -i ~/.ssh/id_ed25519 \
    ec2-user@"$ELASTIC_IP" \
    'echo SSH OK'
```

Note: Amazon Linux 2023 default user is `ec2-user`.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hardcode AMI ID | SSM parameter resolution | ~2019 (AL2) | Never need to update script for new AMIs |
| t2.micro free tier | t3.micro (locked requirement) | 2018 | t3 has better baseline CPU, EBS only, no instance store |
| Amazon Linux 2 | Amazon Linux 2023 | 2023 | AL2023 ships with newer glibc, dnf (not yum), SELinux permissive by default |
| AWS CLI v1 | AWS CLI v2 | 2020 | v2 required for SSM parameter AMI resolution; `brew install awscli` gets v2 |

**Deprecated/outdated:**
- Amazon Linux 2: Reached end-of-standard-support June 2025; AL2023 is the current standard.
- `yum` package manager: AL2023 uses `dnf`; `yum` still works as an alias but `dnf` is canonical.
- AWS CLI v1 (`pip install awscli`): v1 is maintenance-only; v2 is the standard install.

---

## Open Questions

1. **Default VPC presence in the target account**
   - What we know: Locked decision says default VPC is fine
   - What's unclear: Whether the account's us-east-1 default VPC still exists (it can be deleted)
   - Recommendation: Add `aws ec2 describe-vpcs --filters Name=isDefault,Values=true --query 'Vpcs[0].VpcId'` as a pre-flight check at the top of setup-aws-infra.sh; fail fast with a helpful message if missing.

2. **AWS credentials/profile setup method**
   - What we know: Phase plan includes credentials configuration before provisioning; `aws configure` is the standard approach
   - What's unclear: Whether to use `aws configure` (writes `~/.aws/credentials`) or environment variables (`AWS_ACCESS_KEY_ID`, etc.); this affects how the plan's credential task is worded
   - Recommendation: Use `aws configure` with a named profile (`--profile kiss`) to avoid polluting default credentials; then pass `--profile kiss` in the script or set `AWS_PROFILE=kiss` at the top of the script.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | None (bash script execution + aws CLI queries) |
| Config file | none |
| Quick run command | `aws ec2 describe-instances --filters "Name=tag:Name,Values=kiss-server" --region us-east-1 --query 'Reservations[0].Instances[0].State.Name' --output text` |
| Full suite command | `bash scripts/setup-aws-infra.sh` (idempotent re-run) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INFRA-01 | EC2 t3.micro instance running, SSH accessible | smoke | `aws ec2 describe-instances --filters "Name=tag:Name,Values=kiss-server" "Name=instance-state-name,Values=running" --region us-east-1 --query 'Reservations[0].Instances[0].InstanceId' --output text` + `ssh -o ConnectTimeout=10 ec2-user@$EIP 'echo OK'` | ❌ Wave 0 — manual execution |
| INFRA-02 | Elastic IP associated, survives stop/start | smoke | `aws ec2 describe-addresses --filters "Name=tag:Name,Values=kiss-server" --region us-east-1 --query 'Addresses[0].PublicIp' --output text` | ❌ Wave 0 — manual execution |
| INFRA-03 | SG rules: port 80 open, port 22 open | smoke | `aws ec2 describe-security-groups --filters "Name=group-name,Values=kiss-server-sg" --region us-east-1 --query 'SecurityGroups[0].IpPermissions'` | ❌ Wave 0 — manual execution |

### Sampling Rate
- **Per task commit:** Not applicable — infrastructure provisioning, no automated test suite
- **Per wave merge:** `bash scripts/setup-aws-infra.sh` (idempotent re-run verifies all resources exist)
- **Phase gate:** All three AWS resource describe commands return expected values + SSH test succeeds

### Wave 0 Gaps
- [ ] `scripts/setup-aws-infra.sh` — the setup script itself (primary deliverable)
- [ ] No test framework needed — all verification is via AWS CLI describe commands and live SSH test

---

## Sources

### Primary (HIGH confidence)
- [AWS CLI import-key-pair reference](https://docs.aws.amazon.com/cli/latest/reference/ec2/import-key-pair.html) — `--public-key-material fileb://` syntax, duplicate key behavior
- [Amazon Linux 2023 EC2 guide](https://docs.aws.amazon.com/linux/al2023/ug/ec2.html) — SSM parameter path for latest AL2023 AMI
- [AWS CLI allocate-address reference](https://docs.aws.amazon.com/cli/latest/reference/ec2/allocate-address.html) — output schema, AllocationId field
- [AWS CLI associate-address reference](https://docs.aws.amazon.com/cli/latest/reference/ec2/associate-address.html) — AllocationId requirement for VPC instances
- [AWS CLI run-instances reference](https://docs.aws.amazon.com/cli/latest/reference/ec2/run-instances.html) — full parameter list
- [AWS CLI wait instance-running reference](https://docs.aws.amazon.com/cli/latest/reference/ec2/wait/instance-running.html) — polling interval 15s, max 40 retries
- [AWS CLI EC2 security groups guide](https://docs.aws.amazon.com/cli/v1/userguide/cli-services-ec2-sg.html) — create-security-group, authorize-security-group-ingress syntax
- [AWS CLI EC2 instances guide](https://docs.aws.amazon.com/cli/latest/userguide/cli-services-ec2-instances.html) — launch, tag, describe workflow

### Secondary (MEDIUM confidence)
- [4sysops: Elastic IP bash script patterns](https://4sysops.com/archives/assign-associate-an-elastic-ip-address-to-an-ec2-instance-with-a-bash-script/) — practical bash patterns for EIP management
- [describe-key-pairs + check existence](https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-key-pairs.html) — idempotency pattern for key pair import

### Tertiary (LOW confidence)
- None.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified against current official AWS CLI v2 docs
- Architecture: HIGH — all command patterns pulled from official AWS CLI reference pages
- Pitfalls: HIGH — duplicate rule/key errors confirmed via AWS API documentation; t3 credit mode is documented AWS behavior

**Research date:** 2026-03-11
**Valid until:** 2026-06-11 (stable AWS CLI API; SSM parameter paths are AWS-maintained indefinitely)
