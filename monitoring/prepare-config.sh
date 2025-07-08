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
NODE_EXPORTER_PORT=${NODE_EXPORTER_PORT:-9100}
CADVISOR_PORT=${CADVISOR_PORT:-8080}
ALERTMANAGER_PORT=${ALERTMANAGER_PORT:-9093}
BLACKBOX_EXPORTER_PORT=${BLACKBOX_EXPORTER_PORT:-9115}
GRAFANA_PORT=${GRAFANA_PORT:-3000}

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
PROMETHEUS_TEMPLATE_FILE="$SCRIPT_DIR/prometheus/prometheus.yml.template"
PROMETHEUS_STATIC_FILE="$SCRIPT_DIR/prometheus/prometheus.yml"
PROMETHEUS_OUTPUT_FILE="$SCRIPT_DIR/prometheus/prometheus.yml"

GRAFANA_TEMPLATE_FILE="$SCRIPT_DIR/grafana/datasources/datasource.yml.template"
GRAFANA_STATIC_FILE="$SCRIPT_DIR/grafana/datasources/datasource.yml"
GRAFANA_OUTPUT_FILE="$SCRIPT_DIR/grafana/datasources/datasource.yml"

# Process Prometheus configuration
if [ -f "$PROMETHEUS_TEMPLATE_FILE" ]; then
    print_info "Processing prometheus.yml.template..."

    # Simple sed-based substitution
    sed -e "s/\${EQ_PROMETHEUS_PORT}/${EQ_PROMETHEUS_PORT}/g" \
        -e "s/\${CELESTIA_NODE_PORT}/${CELESTIA_NODE_PORT}/g" \
        -e "s/\${DOCKER_GATEWAY_IP}/${DOCKER_GATEWAY_IP}/g" \
        -e "s/\${NODE_EXPORTER_PORT:-9100}/${NODE_EXPORTER_PORT}/g" \
        -e "s/\${CADVISOR_PORT:-8080}/${CADVISOR_PORT}/g" \
        -e "s/\${ALERTMANAGER_PORT:-9093}/${ALERTMANAGER_PORT}/g" \
        -e "s/\${BLACKBOX_EXPORTER_PORT:-9115}/${BLACKBOX_EXPORTER_PORT}/g" \
        -e "s/\${GRAFANA_PORT:-3000}/${GRAFANA_PORT}/g" \
        "$PROMETHEUS_TEMPLATE_FILE" > "$PROMETHEUS_OUTPUT_FILE"

    print_info "✓ Prometheus template processed successfully"

    # Verify the substitution worked
    if grep -q "localhost:${EQ_PROMETHEUS_PORT}" "$PROMETHEUS_OUTPUT_FILE"; then
        print_info "✓ EQ Service configured to localhost:${EQ_PROMETHEUS_PORT}"
    else
        print_error "✗ Failed to configure EQ Service endpoint"
        exit 1
    fi

    if grep -q "localhost:${CELESTIA_NODE_PORT}" "$PROMETHEUS_OUTPUT_FILE"; then
        print_info "✓ Celestia Node configured to localhost:${CELESTIA_NODE_PORT}"
    else
        print_error "✗ Failed to configure Celestia Node endpoint"
        exit 1
    fi

elif [ -f "$PROMETHEUS_STATIC_FILE" ]; then
    print_info "Using static prometheus.yml configuration"
    print_info "✓ Static Prometheus configuration ready"
else
    print_error "No prometheus configuration found (need either prometheus.yml or prometheus.yml.template)"
    exit 1
fi

# Process Grafana datasource configuration
if [ -f "$GRAFANA_TEMPLATE_FILE" ]; then
    print_info "Processing grafana datasource.yml.template..."

    # Simple sed-based substitution for Grafana datasource
    sed -e "s/\${PROMETHEUS_PORT}/${PROMETHEUS_PORT}/g" \
        "$GRAFANA_TEMPLATE_FILE" > "$GRAFANA_OUTPUT_FILE"

    print_info "✓ Grafana datasource template processed successfully"

    # Verify the substitution worked
    if grep -q "localhost:${PROMETHEUS_PORT}" "$GRAFANA_OUTPUT_FILE"; then
        print_info "✓ Grafana datasource configured to localhost:${PROMETHEUS_PORT}"
    else
        print_error "✗ Failed to configure Grafana datasource endpoint"
        exit 1
    fi

elif [ -f "$GRAFANA_STATIC_FILE" ]; then
    print_info "Using static grafana datasource.yml configuration"
    print_info "✓ Static Grafana datasource configuration ready"
else
    print_error "No grafana datasource configuration found (need either datasource.yml or datasource.yml.template)"
    exit 1
fi

print_info "Configuration preparation complete"
