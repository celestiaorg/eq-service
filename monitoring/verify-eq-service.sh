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
    fi
}

# Function to check if EQ Service is running
check_eq_service_running() {
    print_header "Checking if EQ Service is running..."

    local eq_port="${EQ_PROMETHEUS_PORT:-9091}"

    if lsof -i :$eq_port >/dev/null 2>&1; then
        print_status "✓ EQ Service is listening on port $eq_port"

        # Check if it's binding to all interfaces
        if netstat -tlnp 2>/dev/null | grep ":$eq_port" | grep -q "0.0.0.0"; then
            print_status "✓ EQ Service is binding to all interfaces (0.0.0.0)"
        elif netstat -tlnp 2>/dev/null | grep ":$eq_port" | grep -q "127.0.0.1"; then
            print_error "✗ EQ Service is only binding to localhost (127.0.0.1)"
            print_error "   Please set EQ_PROMETHEUS_SOCKET=0.0.0.0:$eq_port in your .env file"
            return 1
        else
            print_warning "⚠ Could not determine EQ Service binding address"
        fi
    else
        print_error "✗ EQ Service is not running on port $eq_port"
        print_error "   Please start the EQ Service with: just run-debug or just run-release"
        return 1
    fi

    return 0
}

# Function to check EQ Service metrics endpoint
check_eq_service_metrics() {
    print_header "Checking EQ Service metrics endpoint..."

    local eq_port="${EQ_PROMETHEUS_PORT:-9091}"

    # Test from localhost
    if curl -s --max-time 5 "http://localhost:$eq_port/metrics" >/dev/null; then
        print_status "✓ EQ Service metrics endpoint accessible from localhost"

        # Check if metrics contain expected data
        local metrics_output=$(curl -s --max-time 5 "http://localhost:$eq_port/metrics")
        if echo "$metrics_output" | grep -q "eqs_grpc_req_total"; then
            print_status "✓ EQ Service metrics contain eqs_grpc_req_total"
        else
            print_warning "⚠ EQ Service metrics do not contain eqs_grpc_req_total"
        fi

        if echo "$metrics_output" | grep -q "eqs_jobs_attempted_total"; then
            print_status "✓ EQ Service metrics contain eqs_jobs_attempted_total"
        else
            print_warning "⚠ EQ Service metrics do not contain eqs_jobs_attempted_total"
        fi

        if echo "$metrics_output" | grep -q "eqs_zk_proof_wait_time"; then
            print_status "✓ EQ Service metrics contain eqs_zk_proof_wait_time"
        else
            print_warning "⚠ EQ Service metrics do not contain eqs_zk_proof_wait_time"
        fi

    else
        print_error "✗ EQ Service metrics endpoint not accessible from localhost"
        return 1
    fi

    return 0
}

# Function to check Docker network connectivity
check_docker_network_connectivity() {
    print_header "Checking Docker network connectivity..."

    local eq_port="${EQ_PROMETHEUS_PORT:-9091}"

    # Get Docker gateway IP
    local docker_gateway=$(docker network inspect monitoring_monitoring 2>/dev/null | jq -r '.[0].IPAM.Config[0].Gateway' 2>/dev/null)
    if [ -z "$docker_gateway" ] || [ "$docker_gateway" = "null" ]; then
        docker_gateway="172.17.0.1"
        print_warning "Could not determine Docker gateway IP, using fallback: $docker_gateway"
    else
        print_status "Detected Docker gateway IP: $docker_gateway"
    fi

    # Test connectivity from Prometheus container
    if docker exec prometheus wget -O- --timeout=5 --tries=1 "http://$docker_gateway:$eq_port/metrics" >/dev/null 2>&1; then
        print_status "✓ EQ Service accessible from Prometheus container at $docker_gateway:$eq_port"
    else
        print_error "✗ EQ Service not accessible from Prometheus container at $docker_gateway:$eq_port"
        print_error "   Make sure EQ_PROMETHEUS_SOCKET=0.0.0.0:$eq_port in your .env file"
        return 1
    fi

    return 0
}

# Function to check Prometheus scraping
check_prometheus_scraping() {
    print_header "Checking Prometheus scraping..."

    local prometheus_port="${PROMETHEUS_PORT:-9090}"

    # Wait a bit for Prometheus to scrape
    print_status "Waiting 10 seconds for Prometheus to scrape metrics..."
    sleep 10

    # Check if Prometheus can reach the EQ Service target
    local targets_json=$(curl -s "http://localhost:$prometheus_port/api/v1/targets" 2>/dev/null)
    if [ -z "$targets_json" ]; then
        print_error "✗ Cannot reach Prometheus API"
        return 1
    fi

    local eq_target_health=$(echo "$targets_json" | jq -r '.data.activeTargets[] | select(.labels.job == "eq-service") | .health')
    if [ "$eq_target_health" = "up" ]; then
        print_status "✓ EQ Service target is healthy in Prometheus"
    else
        print_error "✗ EQ Service target is not healthy in Prometheus (status: $eq_target_health)"

        # Show the last error
        local last_error=$(echo "$targets_json" | jq -r '.data.activeTargets[] | select(.labels.job == "eq-service") | .lastError')
        if [ "$last_error" != "null" ] && [ -n "$last_error" ]; then
            print_error "   Last error: $last_error"
        fi
        return 1
    fi

    return 0
}

# Function to check metrics in Prometheus
check_prometheus_metrics() {
    print_header "Checking EQ Service metrics in Prometheus..."

    local prometheus_port="${PROMETHEUS_PORT:-9090}"

    # Test for eqs_grpc_req_total
    local grpc_req_result=$(curl -s "http://localhost:$prometheus_port/api/v1/query?query=eqs_grpc_req_total" 2>/dev/null)
    if echo "$grpc_req_result" | jq -r '.data.result | length' | grep -q "^[1-9]"; then
        print_status "✓ eqs_grpc_req_total metric found in Prometheus"
    else
        print_error "✗ eqs_grpc_req_total metric not found in Prometheus"
        return 1
    fi

    # Test for eqs_jobs_attempted_total
    local jobs_attempted_result=$(curl -s "http://localhost:$prometheus_port/api/v1/query?query=eqs_jobs_attempted_total" 2>/dev/null)
    if echo "$jobs_attempted_result" | jq -r '.data.result | length' | grep -q "^[1-9]"; then
        print_status "✓ eqs_jobs_attempted_total metric found in Prometheus"
    else
        print_error "✗ eqs_jobs_attempted_total metric not found in Prometheus"
        return 1
    fi

    # Test for eqs_zk_proof_wait_time
    local zk_proof_result=$(curl -s "http://localhost:$prometheus_port/api/v1/query?query=eqs_zk_proof_wait_time_bucket" 2>/dev/null)
    if echo "$zk_proof_result" | jq -r '.data.result | length' | grep -q "^[1-9]"; then
        print_status "✓ eqs_zk_proof_wait_time_bucket metric found in Prometheus"
    else
        print_warning "⚠ eqs_zk_proof_wait_time_bucket metric not found in Prometheus (may be normal if no ZK proofs generated yet)"
    fi

    return 0
}

# Function to check Grafana dashboard
check_grafana_dashboard() {
    print_header "Checking Grafana dashboard..."

    local grafana_port="${GRAFANA_PORT:-3000}"
    local grafana_user="${GF_SECURITY_ADMIN_USER:-admin}"
    local grafana_pass="${GF_SECURITY_ADMIN_PASSWORD:-admin}"

    # Check if Grafana is accessible
    if curl -s --max-time 5 "http://localhost:$grafana_port/api/health" >/dev/null; then
        print_status "✓ Grafana is accessible on port $grafana_port"
    else
        print_error "✗ Grafana is not accessible on port $grafana_port"
        return 1
    fi

    # Check if dashboard exists
    local dashboard_result=$(curl -s -u "$grafana_user:$grafana_pass" "http://localhost:$grafana_port/api/dashboards/uid/eq-service-dashboard" 2>/dev/null)
    if echo "$dashboard_result" | jq -r '.dashboard.title' 2>/dev/null | grep -q "EQ Service Dashboard"; then
        print_status "✓ EQ Service Dashboard found in Grafana"
        print_status "   Dashboard URL: http://localhost:$grafana_port/d/eq-service-dashboard/eq-service-dashboard"
    else
        print_warning "⚠ EQ Service Dashboard not found in Grafana"
    fi

    return 0
}

# Function to show current metric values
show_current_metrics() {
    print_header "Current EQ Service Metrics:"

    local eq_port="${EQ_PROMETHEUS_PORT:-9091}"

    if ! curl -s --max-time 5 "http://localhost:$eq_port/metrics" >/dev/null; then
        print_error "Cannot fetch metrics from EQ Service"
        return 1
    fi

    local metrics_output=$(curl -s --max-time 5 "http://localhost:$eq_port/metrics")

    echo
    echo "gRPC Requests:"
    echo "$metrics_output" | grep "eqs_grpc_req_total" | head -1
    echo
    echo "Job Metrics:"
    echo "$metrics_output" | grep "eqs_jobs_attempted_total" | head -1
    echo "$metrics_output" | grep "eqs_jobs_finished_total" | head -1
    echo "$metrics_output" | grep "eqs_jobs_errors_total" | head -1
    echo
    echo "ZK Proof Wait Time:"
    echo "$metrics_output" | grep "eqs_zk_proof_wait_time_count" | head -1
    echo "$metrics_output" | grep "eqs_zk_proof_wait_time_sum" | head -1
    echo
}

# Function to show helpful URLs
show_helpful_urls() {
    load_env_vars

    print_header "Helpful URLs:"
    echo "EQ Service Metrics:    http://localhost:${EQ_PROMETHEUS_PORT:-9091}/metrics"
    echo "Prometheus:            http://localhost:${PROMETHEUS_PORT:-9090}"
    echo "Prometheus Targets:    http://localhost:${PROMETHEUS_PORT:-9090}/targets"
    echo "Grafana:               http://localhost:${GRAFANA_PORT:-3000}"
    echo "Grafana Dashboard:     http://localhost:${GRAFANA_PORT:-3000}/d/eq-service-dashboard/eq-service-dashboard"
    echo "Grafana Login:         ${GF_SECURITY_ADMIN_USER:-admin} / ${GF_SECURITY_ADMIN_PASSWORD:-admin}"
    echo
}

# Function to show help
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Verification script for EQ Service monitoring setup

OPTIONS:
    -h, --help          Show this help message
    -s, --service       Check EQ Service only
    -m, --metrics       Check EQ Service metrics endpoint only
    -n, --network       Check Docker network connectivity only
    -p, --prometheus    Check Prometheus scraping only
    -g, --grafana       Check Grafana dashboard only
    --show-metrics      Show current metric values
    --urls              Show helpful URLs

EXAMPLES:
    $0                  Run all checks
    $0 --service        Check only if EQ Service is running
    $0 --show-metrics   Show current metric values
    $0 --urls           Show helpful URLs

EOF
}

# Function to run all checks
run_all_checks() {
    print_header "EQ Service Monitoring Verification"
    print_header "================================="

    load_env_vars

    local failed_checks=0

    # Run all checks
    check_eq_service_running || failed_checks=$((failed_checks + 1))
    echo

    check_eq_service_metrics || failed_checks=$((failed_checks + 1))
    echo

    check_docker_network_connectivity || failed_checks=$((failed_checks + 1))
    echo

    check_prometheus_scraping || failed_checks=$((failed_checks + 1))
    echo

    check_prometheus_metrics || failed_checks=$((failed_checks + 1))
    echo

    check_grafana_dashboard || failed_checks=$((failed_checks + 1))
    echo

    show_current_metrics
    echo

    show_helpful_urls

    # Summary
    print_header "Verification Summary"
    if [ $failed_checks -eq 0 ]; then
        print_status "✓ All checks passed! EQ Service monitoring is working correctly."
        print_status "✓ You should now see eqs_* metrics in Prometheus and Grafana dashboard should display data."
    else
        print_error "✗ $failed_checks check(s) failed. Please fix the issues above."
        return 1
    fi

    return 0
}

# Main function
main() {
    case "${1:-}" in
        -h|--help)
            show_help
            exit 0
            ;;
        -s|--service)
            load_env_vars
            check_eq_service_running
            exit $?
            ;;
        -m|--metrics)
            load_env_vars
            check_eq_service_metrics
            exit $?
            ;;
        -n|--network)
            load_env_vars
            check_docker_network_connectivity
            exit $?
            ;;
        -p|--prometheus)
            load_env_vars
            check_prometheus_scraping
            exit $?
            ;;
        -g|--grafana)
            load_env_vars
            check_grafana_dashboard
            exit $?
            ;;
        --show-metrics)
            load_env_vars
            show_current_metrics
            exit $?
            ;;
        --urls)
            show_helpful_urls
            exit 0
            ;;
        "")
            run_all_checks
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
