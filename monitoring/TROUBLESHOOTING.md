# EQ Service Monitoring Troubleshooting Guide

This document summarizes the fixes applied to resolve EQ Service monitoring issues where `eqs_*` metrics were not appearing in Prometheus and Grafana.

## Issues Fixed

### 1. Network Connectivity Problem

**Problem**: EQ Service was binding to `127.0.0.1:9091` (localhost only), making it inaccessible from Docker containers.

**Solution**: Configure EQ Service to bind to all interfaces:
```bash
# In your .env file
EQ_PROMETHEUS_SOCKET=0.0.0.0:9091
```

**Files Modified**:
- `example.env` - Updated default binding from `127.0.0.1:9091` to `0.0.0.0:9091`

### 2. Docker Network Configuration

**Problem**: Prometheus configuration was using `host.docker.internal` which doesn't resolve in all Docker environments.

**Solution**: Use dynamic Docker gateway IP detection:
- Updated `prepare-config.sh` to detect Docker gateway IP automatically
- Modified `prometheus.yml.template` to use `${DOCKER_GATEWAY_IP}` variable

**Files Modified**:
- `monitoring/prepare-config.sh` - Added dynamic gateway IP detection
- `monitoring/prometheus/prometheus.yml.template` - Use `${DOCKER_GATEWAY_IP}` instead of `host.docker.internal`
- `monitoring/prometheus/prometheus.yml` - Updated to use correct gateway IP

### 3. Metric Name Mismatches

**Problem**: Grafana dashboard and alert rules were using incorrect metric names (missing `_total` suffix).

**Actual EQ Service Metrics**:
- `eqs_grpc_req_total` (not `eqs_grpc_req`)
- `eqs_jobs_attempted_total` (not `eqs_jobs_attempted`)
- `eqs_jobs_finished_total` (not `eqs_jobs_finished`)
- `eqs_jobs_errors_total` (not `eqs_jobs_errors`)
- `eqs_zk_proof_wait_time` (histogram - correct)

**Files Modified**:
- `monitoring/grafana/dashboards/eq-service-dashboard.json` - Fixed all metric references
- `monitoring/prometheus/alert_rules.yml` - Fixed all metric references

## Quick Start After Fixes

1. **Update your .env file**:
   ```bash
   EQ_PROMETHEUS_SOCKET=0.0.0.0:9091
   ```

2. **Restart EQ Service**:
   ```bash
   # Kill existing service if running
   pkill eq-service

   # Start with new configuration
   just run-debug  # or just run-release
   ```

3. **Update monitoring configuration**:
   ```bash
   cd monitoring
   ./prepare-config.sh
   ```

4. **Restart monitoring stack**:
   ```bash
   cd monitoring
   ./monitoring-tools.sh --restart
   ```

5. **Verify everything is working**:
   ```bash
   cd monitoring
   ./verify-eq-service.sh
   ```

## Verification Commands

### Check EQ Service is accessible
```bash
# From host
curl http://localhost:9091/metrics | grep eqs_

# From Docker container
docker exec prometheus wget -O- --timeout=5 http://172.18.0.1:9091/metrics | grep eqs_
```

### Check Prometheus is scraping
```bash
# Check targets
curl -s 'http://localhost:9090/api/v1/targets' | jq '.data.activeTargets[] | select(.labels.job == "eq-service")'

# Check metrics
curl -s 'http://localhost:9090/api/v1/query?query=eqs_grpc_req_total' | jq .
```

### Check Grafana Dashboard
- Open: http://localhost:3000/d/eq-service-dashboard/eq-service-dashboard
- Login: admin/admin (default)

## Common Issues and Solutions

### Issue: EQ Service not accessible from Docker containers
**Symptom**: `dial tcp: lookup host.docker.internal: no such host`
**Solution**: Ensure `EQ_PROMETHEUS_SOCKET=0.0.0.0:9091` and run `./prepare-config.sh`

### Issue: Metrics not appearing in Prometheus
**Symptom**: Empty results when querying `eqs_grpc_req_total`
**Solution**:
1. Check EQ Service is running: `netstat -tlnp | grep 9091`
2. Verify binding: should show `0.0.0.0:9091` not `127.0.0.1:9091`
3. Check Prometheus targets: http://localhost:9090/targets

### Issue: Grafana dashboard shows "No data"
**Symptom**: Dashboard panels show no data or "No data" message
**Solution**:
1. Verify metrics are in Prometheus first
2. Check metric names in dashboard match actual metrics
3. Ensure data source is configured correctly

### Issue: Alert rules not firing
**Symptom**: No alerts even when conditions should be met
**Solution**: Check alert rules use correct metric names with `_total` suffix

## Monitoring Stack URLs

After successful setup:
- **EQ Service Metrics**: http://localhost:9091/metrics
- **Prometheus**: http://localhost:9090
- **Prometheus Targets**: http://localhost:9090/targets
- **Grafana**: http://localhost:3000
- **Grafana Dashboard**: http://localhost:3000/d/eq-service-dashboard/eq-service-dashboard
- **Alertmanager**: http://localhost:9093

## Expected Metrics

When working correctly, you should see these metrics:

```
# HELP eqs_grpc_req_total Total number of gRPC requests served.
# TYPE eqs_grpc_req_total counter
eqs_grpc_req_total 1

# HELP eqs_jobs_attempted_total Total number of jobs started, regardless of status.
# TYPE eqs_jobs_attempted_total counter
eqs_jobs_attempted_total 3

# HELP eqs_jobs_finished_total Total number of jobs completed successfully, including retries.
# TYPE eqs_jobs_finished_total counter
eqs_jobs_finished_total 1

# HELP eqs_zk_proof_wait_time Time taken to wait for ZK proof completion in seconds
# TYPE eqs_zk_proof_wait_time histogram
eqs_zk_proof_wait_time_bucket{le="6.0"} 0
eqs_zk_proof_wait_time_bucket{le="12.0"} 0
# ... more buckets
eqs_zk_proof_wait_time_sum 59.418411757
eqs_zk_proof_wait_time_count 1
```

## Scripts Available

- `monitoring/monitoring-tools.sh` - Start/stop/status monitoring stack
- `monitoring/prepare-config.sh` - Update configuration with dynamic IPs
- `monitoring/verify-eq-service.sh` - Comprehensive verification of monitoring setup
- `monitoring/test-config.sh` - Test configuration files

## Files Changed

### Configuration Files
- `example.env` - Updated default socket binding
- `monitoring/prometheus/prometheus.yml.template` - Dynamic gateway IP
- `monitoring/prometheus/prometheus.yml` - Updated target addresses

### Dashboard and Alerts
- `monitoring/grafana/dashboards/eq-service-dashboard.json` - Fixed metric names
- `monitoring/prometheus/alert_rules.yml` - Fixed metric names

### Scripts
- `monitoring/prepare-config.sh` - Added dynamic IP detection
- `monitoring/verify-eq-service.sh` - New verification script
- `monitoring/TROUBLESHOOTING.md` - This file

## Next Steps

1. Test the monitoring setup with some gRPC requests to the EQ Service
2. Generate some jobs to see job metrics populate
3. Set up alerting channels (email, Slack) in `alertmanager/alertmanager.yml`
4. Customize dashboard panels as needed
5. Set up log aggregation if desired

The monitoring stack should now correctly scrape and display EQ Service metrics!
