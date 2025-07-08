#!/bin/bash

set -e

# Simple script to prepare Prometheus configuration with environment variables
# This script handles the substitution of external service ports in prometheus.yml

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Load environment variables from .env file
if [ -f "$SCRIPT_DIR/../.env" ]; then
    set -a
    source "$SCRIPT_DIR/../.env"
    set +a
    print_info "Loaded environment variables from .env file"
else
    print_warn "No .env file found, using defaults"
fi

# Set default values
EQ_PROMETHEUS_PORT=${EQ_PROMETHEUS_PORT:-9091}
CELESTIA_NODE_PORT=${CELESTIA_NODE_PORT:-26658}

# Determine the Docker gateway IP dynamically
DOCKER_GATEWAY_IP=$(docker network inspect monitoring_monitoring 2>/dev/null | jq -r '.[0].IPAM.Config[0].Gateway' 2>/dev/null)
if [ -z "$DOCKER_GATEWAY_IP" ] || [ "$DOCKER_GATEWAY_IP" = "null" ]; then
    # Fallback to common Docker gateway IP
    DOCKER_GATEWAY_IP="172.17.0.1"
    print_warn "Could not determine Docker gateway IP, using fallback: $DOCKER_GATEWAY_IP"
else
    print_info "Detected Docker gateway IP: $DOCKER_GATEWAY_IP"
fi

print_info "Using ports: EQ_PROMETHEUS_PORT=${EQ_PROMETHEUS_PORT}, CELESTIA_NODE_PORT=${CELESTIA_NODE_PORT}"
print_info "Using Docker gateway IP: ${DOCKER_GATEWAY_IP}"

# Configuration files
TEMPLATE_FILE="$SCRIPT_DIR/prometheus/prometheus.yml.template"
STATIC_FILE="$SCRIPT_DIR/prometheus/prometheus.yml"
OUTPUT_FILE="$SCRIPT_DIR/prometheus/prometheus.yml"

# Check if template exists
if [ -f "$TEMPLATE_FILE" ]; then
    print_info "Processing prometheus.yml.template..."

    # Simple sed-based substitution
    sed "s/\${EQ_PROMETHEUS_PORT}/${EQ_PROMETHEUS_PORT}/g; s/\${CELESTIA_NODE_PORT}/${CELESTIA_NODE_PORT}/g; s/\${DOCKER_GATEWAY_IP}/${DOCKER_GATEWAY_IP}/g" \
        "$TEMPLATE_FILE" > "$OUTPUT_FILE"

    print_info "✓ Template processed successfully"

    # Verify the substitution worked
    if grep -q "${DOCKER_GATEWAY_IP}:${EQ_PROMETHEUS_PORT}" "$OUTPUT_FILE"; then
        print_info "✓ EQ Service configured to ${DOCKER_GATEWAY_IP}:${EQ_PROMETHEUS_PORT}"
    else
        print_error "✗ Failed to configure EQ Service endpoint"
        exit 1
    fi

    if grep -q "${DOCKER_GATEWAY_IP}:${CELESTIA_NODE_PORT}" "$OUTPUT_FILE"; then
        print_info "✓ Celestia Node configured to ${DOCKER_GATEWAY_IP}:${CELESTIA_NODE_PORT}"
    else
        print_error "✗ Failed to configure Celestia Node endpoint"
        exit 1
    fi

elif [ -f "$STATIC_FILE" ]; then
    print_info "Using static prometheus.yml configuration"
    print_info "✓ Static configuration ready"
else
    print_error "No prometheus configuration found (need either prometheus.yml or prometheus.yml.template)"
    exit 1
fi

print_info "Configuration preparation complete"
