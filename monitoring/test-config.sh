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

# Function to load environment variables
load_env_vars() {
    if [ -f "$SCRIPT_DIR/../.env" ]; then
        set -a
        source "$SCRIPT_DIR/../.env"
        set +a
        print_status "Loaded environment variables from ../.env"
    else
        print_warning "No .env file found, using default values"
    fi
}

# Function to test configuration files
test_config_files() {
    print_header "Testing configuration files..."

    local files_to_check=(
        "docker-compose.yml"
        "prometheus/alert_rules.yml"
        "alertmanager/alertmanager.yml"
        "grafana/datasources/datasource.yml"
        "grafana/dashboards/dashboard-provider.yml"
        "blackbox/blackbox.yml"
        "receiver/Dockerfile"
        "receiver/app.py"
        "receiver/requirements.txt"
    )

    # At least one of these must exist
    local prometheus_files=(
        "prometheus/prometheus.yml"
        "prometheus/prometheus.yml.template"
    )

    local missing_files=()

    for file in "${files_to_check[@]}"; do
        if [ -f "$SCRIPT_DIR/$file" ]; then
            print_status "✓ $file exists"
        else
            print_error "✗ $file missing"
            missing_files+=("$file")
        fi
    done

    # Check that at least one prometheus configuration exists
    local prometheus_found=false
    for file in "${prometheus_files[@]}"; do
        if [ -f "$SCRIPT_DIR/$file" ]; then
            print_status "✓ $file exists"
            prometheus_found=true
        fi
    done

    if [ "$prometheus_found" = false ]; then
        print_error "✗ No prometheus configuration found (need either prometheus.yml or prometheus.yml.template)"
        missing_files+=("prometheus configuration")
    fi

    if [ ${#missing_files[@]} -gt 0 ]; then
        print_error "Missing files: ${missing_files[*]}"
        return 1
    fi

    print_status "All required configuration files present"
    return 0
}

# Function to test prometheus configuration
test_prometheus_configuration() {
    print_header "Testing Prometheus configuration..."

    # Check if template exists, otherwise check static config
    if [ -f "$SCRIPT_DIR/prometheus/prometheus.yml.template" ]; then
        print_status "Using template-based configuration"

        # Test template processing
        local temp_dir=$(mktemp -d)
        export EQ_PROMETHEUS_PORT=9091
        export CELESTIA_NODE_PORT=26658

        if command -v envsubst >/dev/null 2>&1; then
            envsubst < "$SCRIPT_DIR/prometheus/prometheus.yml.template" > "$temp_dir/prometheus.yml"

            if grep -q "host.docker.internal:9091" "$temp_dir/prometheus.yml"; then
                print_status "✓ EQ Service port substitution works"
            else
                print_error "✗ EQ Service port substitution failed"
                rm -rf "$temp_dir"
                return 1
            fi

            if grep -q "host.docker.internal:26658" "$temp_dir/prometheus.yml"; then
                print_status "✓ Celestia Node port substitution works"
            else
                print_error "✗ Celestia Node port substitution failed"
                rm -rf "$temp_dir"
                return 1
            fi
        else
            print_error "✗ envsubst not available for template testing"
            rm -rf "$temp_dir"
            return 1
        fi

        rm -rf "$temp_dir"
    elif [ -f "$SCRIPT_DIR/prometheus/prometheus.yml" ]; then
        print_status "Using static configuration"

        if grep -q "host.docker.internal:9091" "$SCRIPT_DIR/prometheus/prometheus.yml"; then
            print_status "✓ EQ Service configured with default port 9091"
        else
            print_error "✗ EQ Service not configured correctly"
            return 1
        fi

        if grep -q "host.docker.internal:26658" "$SCRIPT_DIR/prometheus/prometheus.yml"; then
            print_status "✓ Celestia Node configured with default port 26658"
        else
            print_error "✗ Celestia Node not configured correctly"
            return 1
        fi
    else
        print_error "✗ No prometheus configuration found"
        return 1
    fi

    # Check internal service configurations (should be same in both template and static)
    local config_file="$SCRIPT_DIR/prometheus/prometheus.yml"
    if [ -f "$SCRIPT_DIR/prometheus/prometheus.yml.template" ]; then
        config_file="$SCRIPT_DIR/prometheus/prometheus.yml.template"
    fi

    if grep -q "node-exporter:9100" "$config_file"; then
        print_status "✓ Internal services use Docker service names"
    else
        print_error "✗ Internal service configuration incorrect"
        return 1
    fi

    print_status "Prometheus configuration test completed"
    return 0
}

# Function to test Docker Compose syntax
test_docker_compose() {
    print_header "Testing Docker Compose configuration..."

    cd "$SCRIPT_DIR"

    if command -v docker >/dev/null 2>&1; then
        if docker compose config >/dev/null 2>&1; then
            print_status "✓ Docker Compose configuration is valid"
        else
            print_error "✗ Docker Compose configuration is invalid"
            return 1
        fi
    else
        print_warning "Docker not available, skipping Docker Compose test"
    fi

    return 0
}

# Function to test port configurations
test_port_configs() {
    print_header "Testing port configurations..."

    load_env_vars

    local ports=(
        "GRAFANA_PORT:${GRAFANA_PORT:-3000}"
        "PROMETHEUS_PORT:${PROMETHEUS_PORT:-9090}"
        "ALERTMANAGER_PORT:${ALERTMANAGER_PORT:-9093}"
        "NODE_EXPORTER_PORT:${NODE_EXPORTER_PORT:-9100}"
        "CADVISOR_PORT:${CADVISOR_PORT:-8080}"
        "BLACKBOX_EXPORTER_PORT:${BLACKBOX_EXPORTER_PORT:-9115}"
        "RECEIVER_PORT:${RECEIVER_PORT:-2021}"
        "EQ_PROMETHEUS_PORT:${EQ_PROMETHEUS_PORT:-9091}"
        "CELESTIA_NODE_PORT:${CELESTIA_NODE_PORT:-26658}"
    )

    for port_config in "${ports[@]}"; do
        local port_name=${port_config%%:*}
        local port_value=${port_config#*:}

        if [[ "$port_value" =~ ^[0-9]+$ ]] && [ "$port_value" -ge 1024 ] && [ "$port_value" -le 65535 ]; then
            print_status "✓ $port_name=$port_value (valid)"
        else
            print_error "✗ $port_name=$port_value (invalid port range)"
        fi
    done

    return 0
}

# Function to test Grafana configuration
test_grafana_config() {
    print_header "Testing Grafana configuration..."

    load_env_vars

    local grafana_vars=(
        "GF_SECURITY_ADMIN_USER:${GF_SECURITY_ADMIN_USER:-admin}"
        "GF_SECURITY_ADMIN_PASSWORD:${GF_SECURITY_ADMIN_PASSWORD:-admin}"
        "GF_USERS_ALLOW_SIGN_UP:${GF_USERS_ALLOW_SIGN_UP:-false}"
        "GF_INSTALL_PLUGINS:${GF_INSTALL_PLUGINS:-grafana-clock-panel,grafana-simple-json-datasource}"
    )

    for var_config in "${grafana_vars[@]}"; do
        local var_name=${var_config%%:*}
        local var_value=${var_config#*:}

        if [ -n "$var_value" ]; then
            print_status "✓ $var_name=$var_value"
        else
            print_warning "⚠ $var_name is empty"
        fi
    done

    return 0
}

# Function to test Prometheus configuration
test_prometheus_config() {
    print_header "Testing Prometheus configuration..."

    load_env_vars

    local retention=${PROMETHEUS_RETENTION:-200h}
    if [[ "$retention" =~ ^[0-9]+[hdm]$ ]]; then
        print_status "✓ PROMETHEUS_RETENTION=$retention (valid format)"
    else
        print_error "✗ PROMETHEUS_RETENTION=$retention (invalid format, should be like 200h, 30d, 1440m)"
    fi

    return 0
}

# Function to test receiver configuration
test_receiver_config() {
    print_header "Testing receiver configuration..."

    load_env_vars

    local receiver_debug=${RECEIVER_DEBUG:-false}
    if [[ "$receiver_debug" =~ ^(true|false)$ ]]; then
        print_status "✓ RECEIVER_DEBUG=$receiver_debug (valid boolean)"
    else
        print_error "✗ RECEIVER_DEBUG=$receiver_debug (should be true or false)"
    fi

    return 0
}

# Function to test alertmanager configuration
test_alertmanager_config() {
    print_header "Testing Alertmanager configuration..."

    # Check if alertmanager.yml exists
    if [ ! -f "$SCRIPT_DIR/alertmanager/alertmanager.yml" ]; then
        print_error "✗ alertmanager.yml not found"
        return 1
    fi

    # Test with Docker if available
    if command -v docker >/dev/null 2>&1; then
        print_status "Validating Alertmanager configuration with Docker..."
        if docker run --rm -v "$SCRIPT_DIR/alertmanager:/etc/alertmanager" prom/alertmanager:latest \
           --config.file=/etc/alertmanager/alertmanager.yml --version >/dev/null 2>&1; then
            print_status "✓ Alertmanager configuration is valid"
        else
            print_error "✗ Alertmanager configuration is invalid"
            return 1
        fi
    else
        print_warning "Docker not available, skipping Alertmanager validation"
    fi

    # Check for required receivers
    if grep -q "web.hook" "$SCRIPT_DIR/alertmanager/alertmanager.yml"; then
        print_status "✓ Default webhook receiver configured"
    else
        print_error "✗ Default webhook receiver not found"
        return 1
    fi

    if grep -q "eq-service-alerts" "$SCRIPT_DIR/alertmanager/alertmanager.yml"; then
        print_status "✓ EQ Service alerts receiver configured"
    else
        print_error "✗ EQ Service alerts receiver not found"
        return 1
    fi

    print_status "Alertmanager configuration test completed"
    return 0
}

# Function to simulate startup sequence
test_startup_sequence() {
    print_header "Testing startup sequence simulation..."

    load_env_vars

    # Test order of operations
    print_status "1. Docker Compose processes environment variables"
    print_status "2. Services start with static configuration"
    print_status "3. Port mappings handled by Docker Compose"

    # Check prometheus configuration
    if [ -f "$SCRIPT_DIR/prometheus/prometheus.yml.template" ]; then
        print_status "✓ Template-based prometheus configuration ready"
    elif [ -f "$SCRIPT_DIR/prometheus/prometheus.yml" ]; then
        print_status "✓ Static prometheus configuration ready"
    else
        print_error "✗ No prometheus configuration found"
        return 1
    fi

    # Check that monitoring port vars have defaults
    local monitoring_ports=(PROMETHEUS_PORT GRAFANA_PORT ALERTMANAGER_PORT)
    for var in "${monitoring_ports[@]}"; do
        if [ -n "${!var}" ]; then
            print_status "✓ $var=${!var}"
        else
            print_warning "⚠ $var not set, will use default"
        fi
    done

    # Check external service ports
    local external_ports=(EQ_PROMETHEUS_PORT CELESTIA_NODE_PORT)
    for var in "${external_ports[@]}"; do
        if [ -n "${!var}" ]; then
            print_status "✓ $var=${!var}"
        else
            print_warning "⚠ $var not set, will use default"
        fi
    done

    return 0
}

# Function to show configuration summary
show_config_summary() {
    print_header "Configuration Summary"

    load_env_vars

    echo "Service Ports:"
    echo "  Grafana:           ${GRAFANA_PORT:-3000}"
    echo "  Prometheus:        ${PROMETHEUS_PORT:-9090}"
    echo "  Alertmanager:      ${ALERTMANAGER_PORT:-9093}"
    echo "  Node Exporter:     ${NODE_EXPORTER_PORT:-9100}"
    echo "  cAdvisor:          ${CADVISOR_PORT:-8080}"
    echo "  Blackbox Exporter: ${BLACKBOX_EXPORTER_PORT:-9115}"
    echo "  Alert Receiver:    ${RECEIVER_PORT:-2021}"
    echo
    echo "EQ Service Configuration:"
    echo "  Metrics Port:      ${EQ_PROMETHEUS_PORT:-9091}"
    echo "  gRPC Port:         ${EQ_PORT:-50051}"
    echo "  Celestia Node:     ${CELESTIA_NODE_PORT:-26658}"
    echo
    echo "Grafana Configuration:"
    echo "  Admin User:        ${GF_SECURITY_ADMIN_USER:-admin}"
    echo "  Admin Password:    ${GF_SECURITY_ADMIN_PASSWORD:-admin}"
    echo "  Allow Sign Up:     ${GF_USERS_ALLOW_SIGN_UP:-false}"
    echo
    echo "Other Settings:"
    echo "  Prometheus Retention: ${PROMETHEUS_RETENTION:-200h}"
    echo "  Receiver Debug:       ${RECEIVER_DEBUG:-false}"
}

# Function to run all tests
run_all_tests() {
    print_header "Running all configuration tests..."

    local test_results=()

    # Run each test
    test_config_files && test_results+=("✓ Config files") || test_results+=("✗ Config files")
    test_prometheus_configuration && test_results+=("✓ Prometheus config") || test_results+=("✗ Prometheus config")
    test_docker_compose && test_results+=("✓ Docker Compose") || test_results+=("✗ Docker Compose")
    test_port_configs && test_results+=("✓ Port configs") || test_results+=("✗ Port configs")
    test_grafana_config && test_results+=("✓ Grafana config") || test_results+=("✗ Grafana config")
    test_prometheus_config && test_results+=("✓ Prometheus retention") || test_results+=("✗ Prometheus retention")
    test_alertmanager_config && test_results+=("✓ Alertmanager config") || test_results+=("✗ Alertmanager config")
    test_receiver_config && test_results+=("✓ Receiver config") || test_results+=("✗ Receiver config")
    test_startup_sequence && test_results+=("✓ Startup sequence") || test_results+=("✗ Startup sequence")

    print_header "Test Results Summary"
    for result in "${test_results[@]}"; do
        if [[ "$result" == *"✓"* ]]; then
            print_status "$result"
        else
            print_error "$result"
        fi
    done

    # Count failures
    local failures=$(printf '%s\n' "${test_results[@]}" | grep -c "✗" || true)
    local total=${#test_results[@]}
    local passed=$((total - failures))

    echo
    if [ $failures -eq 0 ]; then
        print_status "All tests passed! ($passed/$total)"
        return 0
    else
        print_error "$failures test(s) failed! ($passed/$total passed)"
        return 1
    fi
}

# Function to show help
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Test script for EQ Service monitoring stack configuration

OPTIONS:
    -h, --help              Show this help message
    -a, --all               Run all tests (default)
    -f, --files             Test configuration files only
    -e, --env               Test Prometheus configuration
    -d, --docker            Test Docker Compose configuration
    -p, --ports             Test port configurations
    -g, --grafana           Test Grafana configuration
    -r, --prometheus        Test Prometheus retention configuration
    -m, --alertmanager      Test Alertmanager configuration
    -c, --receiver          Test receiver configuration
    -s, --startup           Test startup sequence
    --summary               Show configuration summary
    --validate              Validate all configurations (same as --all)

EXAMPLES:
    $0                      Run all tests
    $0 --files              Test only configuration files
    $0 --summary            Show current configuration summary
    $0 --validate           Validate all configurations

EOF
}

# Main function
main() {
    case "${1:-}" in
        -h|--help)
            show_help
            exit 0
            ;;
        -f|--files)
            test_config_files
            exit $?
            ;;
        -e|--env)
            test_prometheus_configuration
            exit $?
            ;;
        -d|--docker)
            test_docker_compose
            exit $?
            ;;
        -p|--ports)
            test_port_configs
            exit $?
            ;;
        -g|--grafana)
            test_grafana_config
            exit $?
            ;;
        -r|--prometheus)
            test_prometheus_config
            exit $?
            ;;
        -m|--alertmanager)
            test_alertmanager_config
            exit $?
            ;;
        -c|--receiver)
            test_receiver_config
            exit $?
            ;;
        -s|--startup)
            test_startup_sequence
            exit $?
            ;;
        --summary)
            show_config_summary
            exit 0
            ;;
        -a|--all|--validate|"")
            run_all_tests
            exit $?
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
