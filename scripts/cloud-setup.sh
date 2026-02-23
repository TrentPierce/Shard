#!/bin/bash
# Shard Cloud Setup Helper
# Automatically configures firewall rules for AWS, GCP, and Azure.

set -e

# Ports to open
# TCP 4001: libp2p swarm
# TCP 9091: HTTP API
# UDP 9090: WebRTC
# UDP 9092: QUIC
PORTS_TCP="4001 9091"
PORTS_UDP="9090 9092"

detect_cloud() {
    if curl -s --connect-timeout 2 http://169.254.169.254/latest/meta-data/ >/dev/null; then
        echo "aws"
    elif curl -s --connect-timeout 2 -H "Metadata-Flavor: Google" http://metadata.google.internal/computeMetadata/v1/ >/dev/null; then
        echo "gcp"
    elif curl -s --connect-timeout 2 -H "Metadata: true" "http://169.254.169.254/metadata/instance?api-version=2021-02-01" >/dev/null; then
        echo "azure"
    else
        echo "unknown"
    fi
}

setup_aws() {
    echo "Detected AWS EC2."
    if ! command -v aws &> /dev/null; then
        echo "Error: AWS CLI not found."
        exit 1
    fi

    INSTANCE_ID=$(curl -s http://169.254.169.254/latest/meta-data/instance-id)
    REGION=$(curl -s http://169.254.169.254/latest/meta-data/placement/region)
    SG_ID=$(aws ec2 describe-instances --instance-ids "$INSTANCE_ID" --region "$REGION" --query "Reservations[0].Instances[0].SecurityGroups[0].GroupId" --output text)

    echo "Configuring Security Group: $SG_ID in $REGION"

    for port in $PORTS_TCP; do
        echo "Opening TCP $port..."
        aws ec2 authorize-security-group-ingress --group-id "$SG_ID" --protocol tcp --port "$port" --cidr 0.0.0.0/0 --region "$REGION" 2>/dev/null || echo "Already open or error."
    done

    for port in $PORTS_UDP; do
        echo "Opening UDP $port..."
        aws ec2 authorize-security-group-ingress --group-id "$SG_ID" --protocol udp --port "$port" --cidr 0.0.0.0/0 --region "$REGION" 2>/dev/null || echo "Already open or error."
    done
}

setup_gcp() {
    echo "Detected GCP."
    if ! command -v gcloud &> /dev/null; then
        echo "Error: gcloud CLI not found."
        exit 1
    fi

    echo "Creating firewall rule 'shard-allow-ingress'..."
    gcloud compute firewall-rules create shard-allow-ingress 
        --allow tcp:4001,tcp:9091,udp:9090,udp:9092 
        --target-tags=shard-node 
        --description="Allow Shard P2P and API traffic" 2>/dev/null || echo "Rule might already exist."
    
    echo "Note: Ensure your instance has the tag 'shard-node'."
    echo "Run: gcloud compute instances add-tags [INSTANCE_NAME] --tags=shard-node"
}

setup_azure() {
    echo "Detected Azure."
    if ! command -v az &> /dev/null; then
        echo "Error: Azure CLI not found."
        exit 1
    fi

    # Assuming resource group and NSG logic requires more inputs or interactive mode
    echo "Azure auto-setup requires resource group context."
    echo "Please run the following manually:"
    echo "az network nsg rule create --name AllowShardTCP --priority 1000 --destination-port-ranges 4001 9091 --protocol Tcp --access Allow ..."
    echo "az network nsg rule create --name AllowShardUDP --priority 1001 --destination-port-ranges 9090 9092 --protocol Udp --access Allow ..."
}

main() {
    CLOUD=$(detect_cloud)
    case "$CLOUD" in
        aws) setup_aws ;;
        gcp) setup_gcp ;;
        azure) setup_azure ;;
        *) echo "Could not detect cloud provider (or not supported). Please configure firewall manually." ;;
    esac
}

main
