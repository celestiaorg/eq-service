#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Function to load environment variables
load_env_vars() {
    if [ -f "$SCRIPT_DIR/../.env" ]; then
        set -a
        source "$SCRIPT_DIR/../.env"
        set +a
    fi
}

# Function to check if dashboard is accessible
check_dashboard_access() {
    local grafana_port="${GRAFANA_PORT:-3000}"
    local grafana_user="${GF_SECURITY_ADMIN_USER:-admin}"
    local grafana_pass="${GF_SECURITY_ADMIN_PASSWORD:-admin}"

    print_header "Checking Dashboard Access"

    # Test if Grafana is accessible
    if curl -s -u "$grafana_user:$grafana_pass" "http://localhost:$grafana_port/api/health" >/dev/null; then
        print_status "✓ Grafana is accessible"
    else
        print_error "✗ Grafana is not accessible"
        return 1
    fi

    # Test if dashboard exists
    local dashboard_response=$(curl -s -u "$grafana_user:$grafana_pass" \
        "http://localhost:$grafana_port/api/dashboards/uid/eq-service-dashboard" 2>/dev/null)

    if echo "$dashboard_response" | jq -e '.dashboard.title' >/dev/null 2>&1; then
        local dashboard_title=$(echo "$dashboard_response" | jq -r '.dashboard.title')
        print_status "✓ Dashboard found: $dashboard_title"
    else
        print_error "✗ Dashboard not found"
        return 1
    fi

    return 0
}

# Function to test dashboard queries
test_dashboard_queries() {
    local grafana_port="${GRAFANA_PORT:-3000}"
    local grafana_user="${GF_SECURITY_ADMIN_USER:-admin}"
    local grafana_pass="${GF_SECURITY_ADMIN_PASSWORD:-admin}"

    print_header "Testing Dashboard Queries"

    # Test basic EQ Service metrics
    local queries=(
        "eqs_grpc_req_total:gRPC Requests"
        "eqs_jobs_attempted_total:Jobs Attempted"
        "eqs_jobs_finished_total:Jobs Finished"
        "up{job=\"eq-service\"}:Service Status"
    )

    local working_queries=0
    local total_queries=${#queries[@]}

    for query_info in "${queries[@]}"; do
        local query="${query_info%%:*}"
        local description="${query_info#*:}"

        echo -n "Testing $description... "

        local result=$(curl -s -u "$grafana_user:$grafana_pass" \
            "http://localhost:$grafana_port/api/datasources/proxy/1/api/v1/query?query=$query" 2>/dev/null)

        if [ $? -eq 0 ]; then
            local result_count=$(echo "$result" | jq '.data.result | length' 2>/dev/null)
            if [ "$result_count" -gt 0 ]; then
                local value=$(echo "$result" | jq -r '.data.result[0].value[1]' 2>/dev/null)
                print_status "✓ ($value)"
                working_queries=$((working_queries + 1))
            else
                print_warning "⚠ (no data)"
            fi
        else
            print_error "✗ (query failed)"
        fi
    done

    echo
    print_status "Working queries: $working_queries / $total_queries"

    if [ $working_queries -gt 0 ]; then
        return 0
    else
        return 1
    fi
}

# Function to show current metric values
show_current_metrics() {
    local grafana_port="${GRAFANA_PORT:-3000}"
    local grafana_user="${GF_SECURITY_ADMIN_USER:-admin}"
    local grafana_pass="${GF_SECURITY_ADMIN_PASSWORD:-admin}"

    print_header "Current Metric Values"

    # Get current values
    local metrics=(
        "eqs_grpc_req_total:gRPC Requests Total"
        "eqs_jobs_attempted_total:Jobs Attempted Total"
        "eqs_jobs_finished_total:Jobs Finished Total"
        "eqs_zk_proof_wait_time_count:ZK Proofs Generated"
        "up{job=\"eq-service\"}:Service Status"
    )

    for metric_info in "${metrics[@]}"; do
        local query="${metric_info%%:*}"
        local description="${metric_info#*:}"

        local result=$(curl -s -u "$grafana_user:$grafana_pass" \
            "http://localhost:$grafana_port/api/datasources/proxy/1/api/v1/query?query=$query" 2>/dev/null)

        if [ $? -eq 0 ]; then
            local value=$(echo "$result" | jq -r '.data.result[0].value[1]' 2>/dev/null)
            if [ "$value" != "null" ] && [ -n "$value" ]; then
                printf "  %-25s: %s\n" "$description" "$value"
            else
                printf "  %-25s: %s\n" "$description" "No data"
            fi
        else
            printf "  %-25s: %s\n" "$description" "Query failed"
        fi
    done
}

# Function to show dashboard tips
show_dashboard_tips() {
    load_env_vars

    print_header "Dashboard Tips"

    echo "1. Dashboard URL:"
    echo "   http://localhost:${GRAFANA_PORT:-3000}/d/eq-service-dashboard/eq-service-dashboard"
    echo
    echo "2. Login Credentials:"
    echo "   Username: ${GF_SECURITY_ADMIN_USER:-admin}"
    echo "   Password: ${GF_SECURITY_ADMIN_PASSWORD:-admin}"
    echo
    echo "3. If panels show 'No data':"
    echo "   - Check if metrics exist: curl http://localhost:${EQ_PROMETHEUS_PORT:-9091}/metrics | grep eqs_"
    echo "   - Check Prometheus targets: http://localhost:${PROMETHEUS_PORT:-9090}/targets"
    echo "   - Fix datasource issues: ./fix-dashboard.sh"
    echo
    echo "4. Rate queries (like 'rate(eqs_grpc_req_total[5m])') need time series data:"
    echo "   - They will show 'No data' until there are multiple data points over time"
    echo "   - Make some gRPC requests and wait a few minutes"
    echo
    echo "5. To generate test data:"
    echo "   - Use grpcurl to make requests to the EQ Service"
    echo "   - Or simply wait for the service to accumulate metrics over time"
}

# Function to run a comprehensive dashboard test
run_comprehensive_test() {
    print_header "EQ Service Dashboard Test"
    print_header "========================="

    load_env_vars

    local failed_tests=0

    # Test dashboard access
    if ! check_dashboard_access; then
        failed_tests=$((failed_tests + 1))
        print_error "Dashboard access failed - cannot continue"
        return 1
    fi

    echo

    # Test dashboard queries
    if ! test_dashboard_queries; then
        failed_tests=$((failed_tests + 1))
    fi

    echo

    # Show current metrics
    show_current_metrics

    echo

    # Show tips
    show_dashboard_tips

    echo

    # Summary
    print_header "Test Summary"
    if [ $failed_tests -eq 0 ]; then
        print_status "✓ Dashboard test completed successfully!"
        print_status "✓ Dashboard should be displaying data correctly"
    else
        print_warning "⚠ Some tests failed, but this may be normal if:"
        print_warning "  - EQ Service hasn't processed requests yet"
        print_warning "  - Service has been running for less than 5 minutes"
        print_warning "  - No ZK proofs have been generated yet"
    fi

    return $failed_tests
}

# Function to show help
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Test script for EQ Service Grafana Dashboard

OPTIONS:
    -h, --help          Show this help message
    -t, --test          Run comprehensive dashboard test (default)
    -a, --access        Check dashboard access only
    -q, --queries       Test dashboard queries only
    -m, --metrics       Show current metric values only
    --tips              Show dashboard tips only

EXAMPLES:
    $0                  Run comprehensive dashboard test
    $0 --access         Check if dashboard is accessible
    $0 --metrics        Show current metric values
    $0 --tips           Show dashboard usage tips

EOF
}

# Main function
main() {
    case "${1:-}" in
        -h|--help)
            show_help
            exit 0
            ;;
        -a|--access)
            load_env_vars
            check_dashboard_access
            exit $?
            ;;
        -q|--queries)
            load_env_vars
            test_dashboard_queries
            exit $?
            ;;
        -m|--metrics)
            load_env_vars
            show_current_metrics
            exit $?
            ;;
        --tips)
            show_dashboard_tips
            exit 0
            ;;
        -t|--test|"")
            run_comprehensive_test
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
