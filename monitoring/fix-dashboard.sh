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

# Function to wait for Grafana to be ready
wait_for_grafana() {
    local grafana_port="${GRAFANA_PORT:-3000}"
    local max_attempts=30
    local attempt=0

    print_status "Waiting for Grafana to be ready on port $grafana_port..."

    while [ $attempt -lt $max_attempts ]; do
        if curl -s -o /dev/null -w "%{http_code}" "http://localhost:$grafana_port/api/health" | grep -q "200"; then
            print_status "Grafana is ready!"
            return 0
        fi

        attempt=$((attempt + 1))
        sleep 2
        echo -n "."
    done

    print_error "Grafana failed to start within timeout"
    return 1
}

# Function to get Prometheus datasource UID from Grafana
get_prometheus_datasource_uid() {
    local grafana_port="${GRAFANA_PORT:-3000}"
    local grafana_user="${GF_SECURITY_ADMIN_USER:-admin}"
    local grafana_pass="${GF_SECURITY_ADMIN_PASSWORD:-admin}"

    local datasource_uid=$(curl -s -u "$grafana_user:$grafana_pass" \
        "http://localhost:$grafana_port/api/datasources" | \
        jq -r '.[] | select(.name == "Prometheus") | .uid' 2>/dev/null)

    if [ -n "$datasource_uid" ] && [ "$datasource_uid" != "null" ]; then
        echo "$datasource_uid"
        return 0
    else
        return 1
    fi
}

# Function to fix dashboard datasource references
fix_dashboard_datasources() {
    local dashboard_file="$SCRIPT_DIR/grafana/dashboards/eq-service-dashboard.json"
    local dashboard_backup="$SCRIPT_DIR/grafana/dashboards/eq-service-dashboard.json.backup"

    print_header "Fixing Grafana dashboard datasource references..."

    # Get the actual datasource UID
    local datasource_uid=$(get_prometheus_datasource_uid)
    if [ $? -ne 0 ]; then
        print_error "Failed to get Prometheus datasource UID from Grafana"
        return 1
    fi

    print_status "Found Prometheus datasource UID: $datasource_uid"

    # Create backup
    cp "$dashboard_file" "$dashboard_backup"
    print_status "Created backup: $dashboard_backup"

    # Replace the datasource UID placeholder
    sed "s/\${DS_PROMETHEUS}/$datasource_uid/g" "$dashboard_backup" > "$dashboard_file"

    print_status "Updated dashboard with correct datasource UID"

    # Verify the replacement worked
    if grep -q "$datasource_uid" "$dashboard_file"; then
        print_status "✓ Dashboard datasource references updated successfully"
    else
        print_error "✗ Failed to update dashboard datasource references"
        # Restore from backup
        cp "$dashboard_backup" "$dashboard_file"
        return 1
    fi

    return 0
}

# Function to reload Grafana dashboard
reload_dashboard() {
    local grafana_port="${GRAFANA_PORT:-3000}"
    local grafana_user="${GF_SECURITY_ADMIN_USER:-admin}"
    local grafana_pass="${GF_SECURITY_ADMIN_PASSWORD:-admin}"

    print_status "Reloading Grafana dashboard..."

    # Get current dashboard
    local dashboard_response=$(curl -s -u "$grafana_user:$grafana_pass" \
        "http://localhost:$grafana_port/api/dashboards/uid/eq-service-dashboard" 2>/dev/null)

    if [ $? -eq 0 ]; then
        # Dashboard exists, force reload by restarting Grafana
        print_status "Dashboard found, restarting Grafana to reload configuration..."
        docker restart grafana >/dev/null 2>&1

        # Wait for Grafana to come back up
        wait_for_grafana

        print_status "✓ Dashboard reloaded successfully"
    else
        print_warning "Dashboard not found or Grafana not accessible"
        return 1
    fi

    return 0
}

# Function to verify dashboard is working
verify_dashboard() {
    local grafana_port="${GRAFANA_PORT:-3000}"
    local grafana_user="${GF_SECURITY_ADMIN_USER:-admin}"
    local grafana_pass="${GF_SECURITY_ADMIN_PASSWORD:-admin}"

    print_header "Verifying dashboard functionality..."

    # Test a simple query through the dashboard
    local test_query="eqs_grpc_req_total"
    local result=$(curl -s -u "$grafana_user:$grafana_pass" \
        "http://localhost:$grafana_port/api/datasources/proxy/1/api/v1/query?query=$test_query" 2>/dev/null)

    if [ $? -eq 0 ]; then
        local result_count=$(echo "$result" | jq '.data.result | length' 2>/dev/null)
        if [ "$result_count" -gt 0 ]; then
            print_status "✓ Dashboard queries are working correctly"
            print_status "✓ Dashboard URL: http://localhost:$grafana_port/d/eq-service-dashboard/eq-service-dashboard"
        else
            print_warning "⚠ Dashboard queries return no data (this may be normal if no metrics exist yet)"
        fi
    else
        print_error "✗ Failed to test dashboard queries"
        return 1
    fi

    return 0
}

# Function to show help
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Fix Grafana dashboard datasource references after Grafana starts

OPTIONS:
    -h, --help          Show this help message
    -w, --wait          Wait for Grafana to be ready before fixing
    -f, --fix           Fix dashboard datasource references only
    -r, --reload        Reload dashboard after fixing
    -v, --verify        Verify dashboard is working
    --no-wait           Don't wait for Grafana (assume it's already running)

EXAMPLES:
    $0                  Wait for Grafana, fix dashboard, reload, and verify
    $0 --no-wait        Fix dashboard without waiting (assume Grafana is running)
    $0 --fix            Only fix datasource references
    $0 --verify         Only verify dashboard is working

EOF
}

# Main function
main() {
    print_header "Grafana Dashboard Datasource Fixer"
    print_header "=================================="

    load_env_vars

    local wait_for_grafana_flag=true
    local fix_dashboard_flag=true
    local reload_dashboard_flag=true
    local verify_dashboard_flag=true

    # Parse command line arguments
    while [ $# -gt 0 ]; do
        case "$1" in
            -h|--help)
                show_help
                exit 0
                ;;
            -w|--wait)
                wait_for_grafana_flag=true
                shift
                ;;
            -f|--fix)
                wait_for_grafana_flag=false
                fix_dashboard_flag=true
                reload_dashboard_flag=false
                verify_dashboard_flag=false
                shift
                ;;
            -r|--reload)
                wait_for_grafana_flag=false
                fix_dashboard_flag=false
                reload_dashboard_flag=true
                verify_dashboard_flag=false
                shift
                ;;
            -v|--verify)
                wait_for_grafana_flag=false
                fix_dashboard_flag=false
                reload_dashboard_flag=false
                verify_dashboard_flag=true
                shift
                ;;
            --no-wait)
                wait_for_grafana_flag=false
                shift
                ;;
            *)
                print_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done

    local failed_steps=0

    # Wait for Grafana if requested
    if [ "$wait_for_grafana_flag" = true ]; then
        wait_for_grafana || failed_steps=$((failed_steps + 1))
    fi

    # Fix dashboard datasource references
    if [ "$fix_dashboard_flag" = true ]; then
        fix_dashboard_datasources || failed_steps=$((failed_steps + 1))
    fi

    # Reload dashboard
    if [ "$reload_dashboard_flag" = true ]; then
        reload_dashboard || failed_steps=$((failed_steps + 1))
    fi

    # Verify dashboard is working
    if [ "$verify_dashboard_flag" = true ]; then
        verify_dashboard || failed_steps=$((failed_steps + 1))
    fi

    # Summary
    print_header "Summary"
    if [ $failed_steps -eq 0 ]; then
        print_status "✓ Dashboard fix completed successfully!"
        print_status "✓ You can now access the working dashboard at:"
        print_status "  http://localhost:${GRAFANA_PORT:-3000}/d/eq-service-dashboard/eq-service-dashboard"
        print_status "  Login: ${GF_SECURITY_ADMIN_USER:-admin} / ${GF_SECURITY_ADMIN_PASSWORD:-admin}"
    else
        print_error "✗ $failed_steps step(s) failed"
        return 1
    fi

    return 0
}

# Run main function with all arguments
main "$@"
