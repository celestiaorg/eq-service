# EQ Service Monitoring Setup Summary

## 🎯 Overview

This comprehensive monitoring setup provides full observability for the EQ Service using industry-standard tools.
The setup includes metrics collection, visualization, alerting, and external dependency monitoring.

## 📁 Directory Structure

```
eq-service/infra/
├── docker-compose.yml                # Main orchestration file
├── start-monitoring.sh               # Startup script with validation
├── README.md                         # Detailed documentation
├── SETUP_SUMMARY.md                  # This file
│
├── prometheus/
│   ├── prometheus.yml                # Prometheus configuration
│   └── alert_rules.yml               # Alert rules for EQ Service
│
├── grafana/
│   ├── datasources/
│   │   └── datasource.yml            # Prometheus datasource config
│   └── dashboards/
│       ├── dashboard-provider.yml    # Dashboard provider config
│       └── eq-service-dashboard.json # Main EQ Service dashboard
│
├── alertmanager/
│   └── alertmanager.yml              # Alert routing configuration
│
├── blackbox/
│   └── blackbox.yml                  # API endpoint monitoring
│
└── receiver/
    ├── Dockerfile                    # Custom alert receiver container
    └── app.py                        # Python webhook receiver
```

## 🚀 Quick Start

### Start the monitoring stack

```bash
cd eq-service/infra
./start-monitoring.sh
```

### Access the services

- **Grafana**: <http://localhost:3000> (admin user info required in `.env`, see [e](.))
- **Prometheus**: <http://localhost:9090>
- **Alertmanager**: <http://localhost:9093>

## 📊 Monitoring Capabilities

### EQ Service Metrics

- ✅ **gRPC Request Rate**: `eqs_grpc_req`
- ✅ **Job Processing**: `eqs_jobs_attempted`, `eqs_jobs_finished`, `eqs_jobs_errors`
- ✅ **ZK Proof Timing**: `eqs_zk_proof_wait_time` (histogram with percentiles)
- ✅ **Service Health**: Uptime and availability monitoring

### System Metrics

- ✅ **CPU Usage**: Per-core and aggregate
- ✅ **Memory Usage**: RAM and swap utilization
- ✅ **Disk Space**: Free space and usage trends
- ✅ **Container Metrics**: Docker container resource usage

### Endpoint Monitoring

- ✅ **Upstream Celestia Node**: Connectivity monitoring
- ✅ **Upstream Succinct ZK Network**: Prover network availability
- ✅ **Localhost Endpoints**: gRPC

## 🔔 Alert Rules

### Critical Alerts

- **EqServiceDown**: Service unreachable for 30s
- **EqServiceHighJobFailureRate**: >50% job failure rate
- **EqServiceVerySlowZkProofGeneration**: >10min proof time
- **CelestiaNodeDown**: External dependency failure
- **SuccinctNetworkDown**: ZK prover network failure

### Warning Alerts

- **EqServiceSlowZkProofGeneration**: >5min proof time
- **EqServiceJobsStuck**: Jobs not progressing
- **EqServiceHighMemoryUsage**: >90% memory usage
- **EqServiceHighCpuUsage**: >80% CPU usage
- **EqServiceDiskSpaceLow**: <20% disk space

## 📈 Dashboard Features

### Main EQ Service Dashboard

- **Service Status**: Real-time uptime indicator
- **Request Metrics**: gRPC request rates and patterns
- **Job Processing**: Success/failure rates and queue depth
- **ZK Proof Performance**: Timing histograms and percentiles
- **External Dependencies**: Celestia and Succinct status
- **System Resources**: CPU, memory, and disk usage

### Key Visualizations

- Time series graphs for trends
- Stat panels for current values
- Histogram heatmaps for performance analysis
- Status indicators for service health

## 🛠️ Management Commands

```bash
# Start monitoring stack
./start-monitoring.sh

# Validate configurations only
./start-monitoring.sh --validate

# Check service status
./start-monitoring.sh --status

# Stop all services
./start-monitoring.sh --stop

# Restart services
./start-monitoring.sh --restart

# View logs
./start-monitoring.sh --logs

# Follow logs in real-time
./start-monitoring.sh --logs-follow

# Check EQ Service connectivity
./start-monitoring.sh --check-eq
```

## 🔧 Configuration

### EQ Service Integration

Ensure your EQ Service exposes Prometheus metrics on port 9091 set in .env as `EQ_PROMETHEUS_SOCKET`.

### Custom Metrics

To add custom metrics to your EQ Service:

1. Use the existing `PromMetrics` struct in `prom_metrics.rs`
2. Add new metrics following the established patterns
3. Update `prometheus.yml` if needed for new scrape targets

### Alert Customization

Edit `prometheus/alert_rules.yml` to modify thresholds:

```yaml
- alert: EqServiceSlowZkProofGeneration
  expr: histogram_quantile(0.95, rate(eqs_zk_proof_wait_time_bucket[5m])) > 300
  for: 2m
  labels:
    severity: warning
```

## 📧 Notification Setup

### Email Alerts

Edit `alertmanager/alertmanager.yml`:

```yaml
receivers:
  - name: "email-team"
    email_configs:
      - to: "team@company.com"
        from: "alerts@eq-service.com"
        smarthost: "smtp.gmail.com:587"
        auth_username: "alerts@eq-service.com"
        auth_password: "your-app-password"
```

### Slack Integration

```yaml
receivers:
  - name: "slack-alerts"
    slack_configs:
      - api_url: "https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK"
        channel: "#alerts"
        title: "EQ Service Alert"
```

## 🔒 Security Considerations

### Production Deployment

- [ ] Change default Grafana password
- [ ] Enable TLS for external access
- [ ] Configure proper firewall rules
- [ ] Use secrets management for sensitive data
- [ ] Enable authentication for Prometheus/Alertmanager

### Resource Limits

- [ ] Set appropriate memory limits in docker-compose.yml
- [ ] Configure data retention policies
- [ ] Monitor disk usage for metric storage
- [ ] Set up log rotation

## 🧪 Testing

### Validate Setup

```bash
# Test Prometheus targets
curl http://localhost:9090/api/v1/targets

# Test Grafana API
curl http://admin:admin@localhost:3000/api/health

# Test alert receiver
curl -X POST http://localhost:2021/webhook \
  -H "Content-Type: application/json" \
  -d '{"alerts": [{"status": "firing", "labels": {"alertname": "test"}}]}'
```

### Simulate Alerts

```bash
# Stop EQ Service to trigger alerts
# Monitor logs: docker-compose logs receiver
```

## 📚 References

- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
- [Grafana Dashboard Best Practices](https://grafana.com/docs/grafana/latest/best-practices/)
- [Alertmanager Configuration](https://prometheus.io/docs/alerting/configuration/)
- [EQ Service Metrics](../service/src/internal/prom_metrics.rs)

## 🆘 Troubleshooting

### Common Issues

1. **EQ Service not found**: Check if service is running on port 9091
2. **Permission errors**: Ensure Docker has proper permissions
3. **Port conflicts**: Check if ports 3000, 9090, 9093 are available
4. **Memory issues**: Ensure at least 2GB RAM available

### Debug Commands

```bash
# Check container status
docker-compose ps

# View specific service logs
docker-compose logs prometheus
docker-compose logs grafana

# Check resource usage
docker stats
```

## ✅ Verification Checklist

- [ ] All containers started successfully
- [ ] Grafana accessible with default credentials
- [ ] Prometheus showing EQ Service as target
- [ ] Dashboard displays EQ Service metrics
- [ ] Alerts configured and firing when expected
- [ ] External dependency monitoring working
- [ ] Alert receiver logging notifications

---

**Status**: ✅ Complete monitoring setup with comprehensive observability
**Last Updated**: Created with histogram metric for ZK proof timing
**Next Steps**: Deploy EQ Service and verify metric collection
