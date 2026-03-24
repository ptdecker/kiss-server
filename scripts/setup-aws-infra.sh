#!/usr/bin/env bash
set -euo pipefail

# setup-aws-infra.sh
# Idempotent script to provision EC2 t3.micro, Security Group, and Elastic IP
# for the kiss-server deployment in us-east-1.
#
# Usage: bash scripts/setup-aws-infra.sh
# Requirements: AWS CLI v2 configured with the 'kiss' profile
#
# Resources provisioned:
#   - Key pair: kiss-server (imported from ~/.ssh/id_ed25519.pub)
#   - Security Group: kiss-server-sg (ports 22 and 80 open to 0.0.0.0/0)
#   - EC2 instance: t3.micro, Amazon Linux 2023, tagged Name=kiss-server
#   - Elastic IP: allocated and associated with the instance

REGION="us-east-1"
KEY_NAME="kiss-server"
SG_NAME="kiss-server-sg"
INSTANCE_NAME="kiss-server"
PROFILE="kiss"

# ─── Step 1: Key pair ─────────────────────────────────────────────────────────

echo "==> Step 1: Key pair"

EXISTING_KEY=$(aws ec2 describe-key-pairs \
  --key-names "$KEY_NAME" \
  --profile "$PROFILE" \
  --region "$REGION" \
  --query 'KeyPairs[0].KeyName' \
  --output text 2>/dev/null || echo "")

if [ "$EXISTING_KEY" = "$KEY_NAME" ]; then
  echo "  Key pair '$KEY_NAME' already exists, skipping."
else
  echo "  Importing key pair '$KEY_NAME'..."
  aws ec2 import-key-pair \
    --key-name "$KEY_NAME" \
    --public-key-material "fileb://$HOME/.ssh/id_ed25519.pub" \
    --profile "$PROFILE" \
    --region "$REGION" \
    --output text > /dev/null
  echo "  Key pair '$KEY_NAME' imported."
fi

# ─── Step 2: Security group + rules ───────────────────────────────────────────

echo "==> Step 2: Security group"

EXISTING_SG=$(aws ec2 describe-security-groups \
  --filters "Name=group-name,Values=$SG_NAME" \
  --query 'SecurityGroups[0].GroupId' \
  --output text \
  --profile "$PROFILE" \
  --region "$REGION" 2>/dev/null || echo "")

if [ -z "$EXISTING_SG" ] || [ "$EXISTING_SG" = "None" ]; then
  echo "  Creating security group '$SG_NAME'..."
  SG_ID=$(aws ec2 create-security-group \
    --group-name "$SG_NAME" \
    --description "kiss-server web and SSH access" \
    --profile "$PROFILE" \
    --region "$REGION" \
    --query 'GroupId' \
    --output text)
  echo "  Security group created: $SG_ID"

  echo "  Authorizing port 22 (SSH)..."
  aws ec2 authorize-security-group-ingress \
    --group-id "$SG_ID" \
    --protocol tcp \
    --port 22 \
    --cidr 0.0.0.0/0 \
    --profile "$PROFILE" \
    --region "$REGION" \
    --output text > /dev/null

  echo "  Authorizing port 80 (HTTP)..."
  aws ec2 authorize-security-group-ingress \
    --group-id "$SG_ID" \
    --protocol tcp \
    --port 80 \
    --cidr 0.0.0.0/0 \
    --profile "$PROFILE" \
    --region "$REGION" \
    --output text > /dev/null

  echo "  Security group '$SG_NAME' configured with ports 22 and 80."
else
  SG_ID="$EXISTING_SG"
  echo "  Security group '$SG_NAME' already exists: $SG_ID, skipping."
fi

# ─── Step 3: AMI lookup via SSM ───────────────────────────────────────────────

echo "==> Step 3: AMI lookup"

AMI_ID=$(aws ssm get-parameter \
  --name /aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64 \
  --profile "$PROFILE" \
  --region "$REGION" \
  --query 'Parameter.Value' \
  --output text)
echo "  Using AMI: $AMI_ID"

# ─── Step 4: EC2 instance ─────────────────────────────────────────────────────

echo "==> Step 4: EC2 instance"

EXISTING_INSTANCE=$(aws ec2 describe-instances \
  --filters "Name=tag:Name,Values=$INSTANCE_NAME" "Name=instance-state-name,Values=running,stopped,pending" \
  --query 'Reservations[0].Instances[0].InstanceId' \
  --output text \
  --profile "$PROFILE" \
  --region "$REGION" 2>/dev/null || echo "")

if [ -n "$EXISTING_INSTANCE" ] && [ "$EXISTING_INSTANCE" != "None" ]; then
  INSTANCE_ID="$EXISTING_INSTANCE"
  echo "  Instance '$INSTANCE_NAME' already exists: $INSTANCE_ID, skipping."
else
  echo "  Launching instance '$INSTANCE_NAME'..."
  INSTANCE_ID=$(aws ec2 run-instances \
    --image-id "$AMI_ID" \
    --instance-type t3.micro \
    --key-name "$KEY_NAME" \
    --security-group-ids "$SG_ID" \
    --count 1 \
    --credit-specification CpuCredits=standard \
    --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$INSTANCE_NAME}]" \
    --profile "$PROFILE" \
    --region "$REGION" \
    --query 'Instances[0].InstanceId' \
    --output text)
  echo "  Launched instance: $INSTANCE_ID"
  echo "  Waiting for instance to reach running state (this takes 30–90 seconds)..."
  aws ec2 wait instance-running \
    --instance-ids "$INSTANCE_ID" \
    --profile "$PROFILE" \
    --region "$REGION"
  echo "  Instance is running."
fi

# ─── Step 5: Elastic IP ───────────────────────────────────────────────────────

echo "==> Step 5: Elastic IP"

EXISTING_ALLOC=$(aws ec2 describe-addresses \
  --filters "Name=tag:Name,Values=$INSTANCE_NAME" \
  --query 'Addresses[0].AllocationId' \
  --output text \
  --profile "$PROFILE" \
  --region "$REGION" 2>/dev/null || echo "")

if [ -z "$EXISTING_ALLOC" ] || [ "$EXISTING_ALLOC" = "None" ]; then
  echo "  Allocating new Elastic IP..."
  ALLOC_ID=$(aws ec2 allocate-address \
    --domain vpc \
    --profile "$PROFILE" \
    --region "$REGION" \
    --query 'AllocationId' \
    --output text)
  echo "  Allocated: $ALLOC_ID"

  echo "  Tagging Elastic IP..."
  aws ec2 create-tags \
    --resources "$ALLOC_ID" \
    --tags "Key=Name,Value=$INSTANCE_NAME" \
    --profile "$PROFILE" \
    --region "$REGION"
else
  ALLOC_ID="$EXISTING_ALLOC"
  echo "  Elastic IP already exists: $ALLOC_ID, skipping allocation."
fi

echo "  Associating Elastic IP with instance $INSTANCE_ID..."
aws ec2 associate-address \
  --instance-id "$INSTANCE_ID" \
  --allocation-id "$ALLOC_ID" \
  --profile "$PROFILE" \
  --region "$REGION" \
  --output text > /dev/null

ELASTIC_IP=$(aws ec2 describe-addresses \
  --allocation-ids "$ALLOC_ID" \
  --query 'Addresses[0].PublicIp' \
  --output text \
  --profile "$PROFILE" \
  --region "$REGION")

# ─── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "==> Infrastructure ready"
echo "  Instance ID:    $INSTANCE_ID"
echo "  Security Group: $SG_ID ($SG_NAME)"
echo "  Key Pair:       $KEY_NAME"
echo "  Elastic IP:     $ELASTIC_IP"
echo ""
echo "Elastic IP: $ELASTIC_IP"
