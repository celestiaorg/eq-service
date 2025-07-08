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

# Function to test a Grafana query
test_grafana_query() {
    local query=$1
    local description=$2
    local grafana_port="${GRAFANA_PORT:-3000}"
    local grafana_user="${GF_SECURITY_ADMIN_USER:-admin}"
    local grafana_pass="${GF_SECURITY_ADMIN_PASSWORD:-admin}"

    echo -n "Testing: $description... "

    # Use datasource proxy to test the query
    local result=$(curl -s -u "$grafana_user:$grafana_pass" \
        "http://localhost:$grafana_port/api/datasources/proxy/1/api/v1/query?query=$query" 2>/dev/null)

    if [ $? -eq 0 ]; then
        local result_count=$(echo "$result" | jq '.data.result | length' 2>/dev/null)
        if [ "$result_count" -gt 0 ]; then
            print_status "✓ ($result_count results)"
        else
            print_warning "⚠ (0 results)"
        fi
    else
        print_error "✗ (query failed)"
        return 1
    fi

    return 0
}

# Function to test rate queries
test_rate_query() {
    local base_metric=$1
    local description=$2
    local rate_query="rate(${base_metric}[5m])"

    test_grafana_query "$rate_query" "$description (rate)"
}

# Function to test histogram queries
test_histogram_query() {
    local base_metric=$1
    local description=$2
    local percentile_query="histogram_quantile(0.50, rate(${base_metric}_bucket[5m]))"

    test_grafana_query "$percentile_query" "$description (50th percentile)"
}

# Function to run all dashboard query tests
run_dashboard_tests() {
    print_header "Testing Grafana Dashboard Queries"
    print_header "================================="

    load_env_vars

    local failed_tests=0

    # Test basic metrics
    print_header "Basic Metrics"
    test_grafana_query "eqs_grpc_req_total" "gRPC Requests Total" || failed_tests=$((failed_tests + 1))
    test_grafana_query "eqs_jobs_attempted_total" "Jobs Attempted Total" || failed_tests=$((failed_tests + 1))
    test_grafana_query "eqs_jobs_finished_total" "Jobs Finished Total" || failed_tests=$((failed_tests + 1))
    test_grafana_query "eqs_jobs_errors_total" "Jobs Errors Total" || failed_tests=$((failed_tests + 1))
    test_grafana_query "eqs_zk_proof_wait_time_count" "ZK Proof Wait Time Count" || failed_tests=$((failed_tests + 1))
    echo

    # Test rate queries (like in the dashboard)
    print_header "Rate Queries (Dashboard Panels)"
    test_rate_query "eqs_grpc_req_total" "gRPC Request Rate" || failed_tests=$((failed_tests + 1))
    test_rate_query "eqs_jobs_attempted_total" "Jobs Attempted Rate" || failed_tests=$((failed_tests + 1))
    test_rate_query "eqs_jobs_finished_total" "Jobs Finished Rate" || failed_tests=$((failed_tests + 1))
    test_rate_query "eqs_jobs_errors_total" "Jobs Error Rate" || failed_tests=$((failed_tests + 1))
    echo

    # Test histogram queries
    print_header "Histogram Queries (ZK Proof Timing)"
    test_histogram_query "eqs_zk_proof_wait_time" "ZK Proof Wait Time" || failed_tests=$((failed_tests + 1))
    test_grafana_query "histogram_quantile(0.95, rate(eqs_zk_proof_wait_time_bucket[5m]))" "ZK Proof Wait Time (95th percentile)" || failed_tests=$((failed_tests + 1))
    test_grafana_query "histogram_quantile(0.99, rate(eqs_zk_proof_wait_time_bucket[5m]))" "ZK Proof Wait Time (99th percentile)" || failed_tests=$((failed_tests + 1))
    echo

    # Test calculated metrics
    print_header "Calculated Metrics"
    test_grafana_query "eqs_jobs_attempted_total - eqs_jobs_finished_total - eqs_jobs_errors_total" "Jobs in Progress" || failed_tests=$((failed_tests + 1))
    test_grafana_query "rate(eqs_jobs_errors_total[5m]) / rate(eqs_jobs_attempted_total[5m])" "Job Failure Rate" || failed_tests=$((failed_tests + 1))
    echo

    # Test service health
    print_header "Service Health"
    test_grafana_query "up{job=\"eq-service\"}" "EQ Service Status" || failed_tests=$((failed_tests + 1))
    echo

    # Test external dependencies
    print_header "External Dependencies"
    test_grafana_query "probe_success{instance=\"https://docs.celestia.org\"}" "Celestia Network Status" || failed_tests=$((failed_tests + 1))
    test_grafana_query "probe_success{instance=\"https://api.succinct.xyz\"}" "Succinct Network Status" || failed_tests=$((failed_tests + 1))
    echo

    # Test system metrics
    print_header "System Metrics"
    test_grafana_query "100 - (avg by(instance) (irate(node_cpu_seconds_total{mode=\"idle\"}[5m])) * 100)" "CPU Usage" || failed_tests=$((failed_tests + 1))
    test_grafana_query "(node_memory_MemTotal_bytes - node_memory_MemAvailable_bytes) / node_memory_MemTotal_bytes * 100" "Memory Usage" || failed_tests=$((failed_tests + 1))
    echo

    # Summary
    print_header "Test Summary"
    if [ $failed_tests -eq 0 ]; then
        print_status "✓ All dashboard queries are working correctly!"
        print_status "✓ You can now access the Grafana dashboard at: http://localhost:${GRAFANA_PORT:-3000}/d/eq-service-dashboard/eq-service-dashboard"
    else
        print_error "✗ $failed_tests query test(s) failed"
        print_error "  This may be normal if the EQ Service hasn't processed any requests yet"
        print_error "  Try making some gRPC requests to the EQ Service and run this test again"
    fi

    echo
    print_header "Helpful Commands"
    echo "Generate some test data:"
    echo "  curl -s http://localhost:9091/metrics | grep eqs_"
    echo
    echo "Access Grafana Dashboard:"
    echo "  http://localhost:${GRAFANA_PORT:-3000}/d/eq-service-dashboard/eq-service-dashboard"
    echo "  Login: ${GF_SECURITY_ADMIN_USER:-admin} / ${GF_SECURITY_ADMIN_PASSWORD:-admin}"
    echo
    echo "Test individual queries in Grafana:"
    echo "  Go to Explore -> Select Prometheus -> Enter query: eqs_grpc_req_total"
    echo

    return $failed_tests
}

# Function to show current metric values
show_current_values() {
    print_header "Current EQ Service Metric Values"
    print_header "================================"

    load_env_vars

    local grafana_port="${GRAFANA_PORT:-3000}"
    local grafana_user="${GF_SECURITY_ADMIN_USER:-admin}"
    local grafana_pass="${GF_SECURITY_ADMIN_PASSWORD:-admin}"

    # Function to get and display metric value
    get_metric_value() {
        local query=$1
        local description=$2

        local result=$(curl -s -u "$grafana_user:$grafana_pass" \
            "http://localhost:$grafana_port/api/datasources/proxy/1/api/v1/query?query=$query" 2>/dev/null)

        local value=$(echo "$result" | jq -r '.data.result[0].value[1]' 2>/dev/null)

        if [ "$value" != "null" ] && [ -n "$value" ]; then
            printf "%-30s: %s\n" "$description" "$value"
        else
            printf "%-30s: %s\n" "$description" "No data"
        fi
    }

    echo
    get_metric_value "eqs_grpc_req_total" "gRPC Requests Total"
    get_metric_value "eqs_jobs_attempted_total" "Jobs Attempted"
    get_metric_value "eqs_jobs_finished_total" "Jobs Finished"
    get_metric_value "eqs_jobs_errors_total" "Jobs Failed"
    get_metric_value "eqs_zk_proof_wait_time_count" "ZK Proofs Generated"
    get_metric_value "eqs_zk_proof_wait_time_sum" "ZK Proof Total Wait Time"
    echo
    get_metric_value "rate(eqs_grpc_req_total[5m])" "gRPC Request Rate (/sec)"
    get_metric_value "rate(eqs_jobs_attempted_total[5m])" "Job Attempt Rate (/sec)"
    get_metric_value "up{job=\"eq-service\"}" "Service Status (1=up, 0=down)"
    echo
}

# Function to show help
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Test script for Grafana dashboard queries

OPTIONS:
    -h, --help          Show this help message
    -t, --test          Run all dashboard query tests (default)
    -v, --values        Show current metric values
    -q, --query QUERY   Test a specific query

EXAMPLES:
    $0                  Run all dashboard query tests
    $0 --values         Show current metric values
    $0 --query "eqs_grpc_req_total"  Test a specific query

EOF
}

# Main function
main() {
    case "${1:-}" in
        -h|--help)
            show_help
            exit 0
            ;;
        -v|--values)
            show_current_values
            exit 0
            ;;
        -q|--query)
            if [ -z "$2" ]; then
                print_error "Query parameter is required"
                show_help
                exit 1
            fi
            load_env_vars
            test_grafana_query "$2" "Custom Query"
            exit $?
            ;;
        -t|--test|"")
            run_dashboard_tests
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
