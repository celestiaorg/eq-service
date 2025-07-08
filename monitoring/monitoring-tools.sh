#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo -e "${BLUE}[HEADER]${NC} $1"
}

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check if a port is in use
port_in_use() {
    lsof -i :$1 >/dev/null 2>&1
}

# Function to wait for service to be ready
wait_for_service() {
    local service_name=$1
    local port=$2
    local max_attempts=30
    local attempt=0

    print_status "Waiting for $service_name to be ready on port $port..."

    while [ $attempt -lt $max_attempts ]; do
        if [ "$service_name" = "Alertmanager" ]; then
            # Use v2 API for Alertmanager
            if curl -s -o /dev/null -w "%{http_code}" http://localhost:$port/api/v2/status | grep -q "200"; then
                print_status "$service_name is ready!"
                return 0
            fi
        else
            if curl -s -o /dev/null -w "%{http_code}" http://localhost:$port > /dev/null 2>&1; then
                print_status "$service_name is ready!"
                return 0
            fi
        fi

        attempt=$((attempt + 1))
        sleep 2
        echo -n "."
    done

    print_error "$service_name failed to start within timeout"
    return 1
}

# Function to load environment variables
load_env_vars() {
    if [ -f "$SCRIPT_DIR/../.env" ]; then
        set -a
        source "$SCRIPT_DIR/../.env"
        set +a
    fi
}

# Function to validate configuration files
validate_configs() {
    print_header "Validating configuration files..."

    # Check if required files exist
    local required_files=(
        "docker-compose.yml"
        "prometheus/prometheus.yml"
        "prometheus/alert_rules.yml"
        "alertmanager/alertmanager.yml"
        "grafana/datasources/datasource.yml"
        "blackbox/blackbox.yml"
    )

    for file in "${required_files[@]}"; do
        if [ ! -f "$SCRIPT_DIR/$file" ]; then
            print_error "Required file not found: $file"
            exit 1
        fi
    done

    # Validate Prometheus configuration
    if command_exists docker; then
        print_status "Validating Prometheus configuration..."
        if ! docker run --rm \
            -v "$SCRIPT_DIR/prometheus:/etc/prometheus:ro" \
            --entrypoint /bin/promtool \
            prom/prometheus:latest \
            check config /etc/prometheus/prometheus.yml >/dev/null; then
            print_error "Prometheus configuration validation failed"
            exit 1
        fi

        print_status "Validating Prometheus alert rules..."
        if ! docker run --rm \
            -v "$SCRIPT_DIR/prometheus:/etc/prometheus:ro" \
            --entrypoint /bin/promtool \
            prom/prometheus:latest \
            check rules /etc/prometheus/alert_rules.yml >/dev/null 2>&1; then
            print_error "Prometheus alert rules validation failed"
            exit 1
        fi
    fi

    print_status "All configuration files validated successfully"
}

# Function to check prerequisites
check_prerequisites() {
    print_header "Checking prerequisites..."

    # Check Docker
    if ! command_exists docker; then
        print_error "Docker is not installed or not in PATH"
        exit 1
    fi

    # Check Docker Compose
    if ! command_exists docker compose; then
        print_error "Docker Compose is not installed or not in PATH"
        exit 1
    fi

    # Check Docker daemon
    if ! docker info >/dev/null 2>&1; then
        print_error "Docker daemon is not running"
        exit 1
    fi

    # Check available memory
    local available_memory=$(free -m | awk '/^Mem:/ {print $7}')
    if [ "$available_memory" -lt 2048 ]; then
        print_warning "Available memory is less than 2GB. The monitoring stack may not perform well."
    fi

    # Load environment variables from .env file
    if [ -f "$SCRIPT_DIR/../.env" ]; then
        set -a
        source "$SCRIPT_DIR/../.env"
        set +a
    fi

    # Check port availability using environment variables
    local ports=(
        "${GRAFANA_PORT:-3000}"
        "${PROMETHEUS_PORT:-9090}"
        "${ALERTMANAGER_PORT:-9093}"
        "${NODE_EXPORTER_PORT:-9100}"
        "${CADVISOR_PORT:-8080}"
        "${BLACKBOX_EXPORTER_PORT:-9115}"
        "${RECEIVER_PORT:-2021}"
    )
    local ports_in_use=()

    for port in "${ports[@]}"; do
        if port_in_use $port; then
            ports_in_use+=($port)
        fi
    done

    if [ ${#ports_in_use[@]} -gt 0 ]; then
        print_warning "The following ports are already in use: ${ports_in_use[*]}"
        print_warning "This may cause conflicts with the monitoring stack"
        echo -n "Do you want to continue? (y/N): "
        read -r response
        if [[ ! "$response" =~ ^[Yy]$ ]]; then
            print_status "Aborted by user"
            exit 0
        fi
    fi

    print_status "All prerequisites check passed"
}

# Function to start services
start_services() {
    print_header "Starting monitoring stack..."

    cd "$SCRIPT_DIR"

    # Prepare configuration
    print_status "Preparing configuration..."
    ./prepare-config.sh

    # Pull latest images
    print_status "Pulling latest Docker images..."
    docker compose pull

    # Start services
    print_status "Starting services..."
    docker compose up -d

    # Load environment variables
    load_env_vars

    # Wait for services to be ready
    wait_for_service "Prometheus" "${PROMETHEUS_PORT:-9090}"
    wait_for_service "Grafana" "${GRAFANA_PORT:-3000}"
    wait_for_service "Alertmanager" "${ALERTMANAGER_PORT:-9093}"

    print_status "All services started successfully!"
}

# Function to show service status
show_status() {
    load_env_vars

    print_header "Service Status"
    docker compose ps
    echo

    print_header "Service URLs"
    echo "Grafana:      http://localhost:${GRAFANA_PORT:-3000} (${GF_SECURITY_ADMIN_USER:-admin}/${GF_SECURITY_ADMIN_PASSWORD:-admin})"
    echo "Prometheus:   http://localhost:${PROMETHEUS_PORT:-9090}"
    echo "Alertmanager: http://localhost:${ALERTMANAGER_PORT:-9093}"
    echo "Receiver:     http://localhost:${RECEIVER_PORT:-2021}"
    echo

    print_header "Monitoring Targets"
    echo "EQ Service:   http://host.docker.internal:${EQ_PROMETHEUS_PORT:-9091}/metrics"
    echo "Node Exporter: http://localhost:${NODE_EXPORTER_PORT:-9100}/metrics"
    echo "cAdvisor:     http://localhost:${CADVISOR_PORT:-8080}/metrics"
    echo
}

# Function to check EQ Service connectivity
check_eq_service() {
    load_env_vars

    print_header "Checking EQ Service connectivity..."

    # Try to reach EQ Service metrics endpoint
    local eq_port="${EQ_PROMETHEUS_PORT:-9091}"
    if curl -s -o /dev/null -w "%{http_code}" http://localhost:$eq_port/metrics | grep -q "200"; then
        print_status "EQ Service metrics endpoint is accessible"
    else
        print_warning "EQ Service metrics endpoint is not accessible at http://localhost:$eq_port/metrics"
        print_warning "Make sure your EQ Service is running and exposing metrics"
    fi
}

# Function to show help
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

OPTIONS:
    -h, --help          Show this help message
    -v, --validate      Validate configuration files only
    -s, --status        Show service status
    -c, --check-eq      Check EQ Service connectivity
    --stop              Stop all services
    --restart           Restart all services
    --logs              Show logs from all services
    --logs-follow       Follow logs from all services

EXAMPLES:
    $0                  Start the monitoring stack
    $0 --validate       Validate configurations without starting
    $0 --status         Show current service status
    $0 --stop           Stop all monitoring services
    $0 --logs           Show recent logs
    $0 --logs-follow    Follow logs in real-time

EOF
}

# Function to stop services
stop_services() {
    print_header "Stopping monitoring stack..."
    cd "$SCRIPT_DIR"
    docker compose down
    print_status "All services stopped"
}

# Function to restart services
restart_services() {
    print_header "Restarting monitoring stack..."
    cd "$SCRIPT_DIR"
    docker compose restart
    print_status "All services restarted"
}

# Function to show logs
show_logs() {
    load_env_vars
    cd "$SCRIPT_DIR"
    if [ "$1" == "follow" ]; then
        docker compose logs -f
    else
        docker compose logs
    fi
}

# Main function
main() {
    print_header "EQ Service Monitoring Stack"
    print_header "================================"

    case "${1:-}" in
        -h|--help)
            show_help
            exit 0
            ;;
        -v|--validate)
            validate_configs
            exit 0
            ;;
        -s|--status)
            show_status
            exit 0
            ;;
        -c|--check-eq)
            check_eq_service
            exit 0
            ;;
        --stop)
            stop_services
            exit 0
            ;;
        --restart)
            print_status "Preparing configuration..."
            ./prepare-config.sh
            restart_services
            exit 0
            ;;
        --logs)
            show_logs
            exit 0
            ;;
        --logs-follow)
            show_logs follow
            exit 0
            ;;
        "")
            # Default action: start monitoring stack
            check_prerequisites
            validate_configs

            # Prepare configuration before starting
            print_status "Preparing configuration..."
            ./prepare-config.sh

            start_services
            show_status
            check_eq_service



            # Load environment variables for final output
            load_env_vars

            echo
            print_status "Monitoring stack is now running!"
            print_status "Access Grafana at: http://localhost:${GRAFANA_PORT:-3000} (${GF_SECURITY_ADMIN_USER:-admin}/${GF_SECURITY_ADMIN_PASSWORD:-admin})"
            print_status "Access Prometheus at: http://localhost:${PROMETHEUS_PORT:-9090}"
            print_status "Access Alertmanager at: http://localhost:${ALERTMANAGER_PORT:-9093}"
            echo
            print_status "To stop the monitoring stack, run: $0 --stop"
            print_status "To view logs, run: $0 --logs"
            ;;
        *)
            print_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
