# EQ Service Monitoring Infrastructure

This directory contains a comprehensive monitoring setup for the EQ Service using Prometheus, Grafana, and Alertmanager. The setup is based on Docker Compose and provides full observability for the EQ Service application.

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   EQ Service    │───▶│   Prometheus    │───▶│    Grafana      │
│   (Port 9091)   │    │   (Port 9090)   │    │   (Port 3000)   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                │
                                ▼
                       ┌─────────────────┐    ┌─────────────────┐
                       │  Alertmanager   │───▶│    Receiver     │
                       │   (Port 9093)   │    │   (Port 2021)   │
                       └─────────────────┘    └─────────────────┘
                                │
                                ▼
                       ┌─────────────────┐
                       │ Node Exporter   │
                       │   (Port 9100)   │
                       └─────────────────┘
```

## Components

### Core Monitoring Stack
- **Prometheus**: Time-series database and monitoring system
- **Grafana**: Visualization and dashboards
- **Alertmanager**: Alert routing and notifications
- **Node Exporter**: System metrics collection
- **cAdvisor**: Container metrics collection
- **Blackbox Exporter**: External endpoint monitoring
- **Receiver**: Custom webhook receiver for alerts

### Monitored Metrics

#### EQ Service Metrics
- `eqs_grpc_req`: gRPC request counter
- `eqs_jobs_attempted`: Total jobs attempted
- `eqs_jobs_finished`: Total jobs completed successfully
- `eqs_jobs_errors`: Total jobs failed (labeled by error type)
- `eqs_zk_proof_wait_time`: ZK proof generation time histogram

#### System Metrics
- CPU usage
- Memory usage
- Disk space
- Network I/O
- Container resource usage

#### External Dependencies
- Celestia network connectivity
- Succinct ZK prover network connectivity

## Quick Start

### Prerequisites
1. Docker and Docker Compose installed
2. EQ Service running and exposing metrics on port 9091
3. At least 4GB RAM available for the monitoring stack

### 1. Start the Monitoring Stack

```bash
# Navigate to the infra directory
cd eq-service/infra

# Start all services
docker-compose up -d

# Check status
docker-compose ps
```

### 2. Access the Services

- **Grafana**: http://localhost:3000
  - Username: `admin`
  - Password: `admin`
- **Prometheus**: http://localhost:9090
- **Alertmanager**: http://localhost:9093
- **Alert Receiver**: http://localhost:2021

### 3. Configure Your EQ Service

Ensure your EQ Service is exposing metrics on port 9091. The monitoring stack expects the service to be accessible at `host.docker.internal:9091` from within the containers.

## Configuration

### Environment Variables

You can customize the setup using environment variables:

```bash
# Create a .env file
cat > .env << EOF
# Grafana Configuration
GF_SECURITY_ADMIN_PASSWORD=your-secure-password
GF_INSTALL_PLUGINS=grafana-clock-panel,grafana-simple-json-datasource

# Alert Receiver Configuration
RECEIVER_DEBUG=false
RECEIVER_PORT=2021

# Prometheus Configuration
PROMETHEUS_RETENTION=200h
EOF
```

### Monitoring Targets

Edit `prometheus/prometheus.yml` to add or modify monitoring targets:

```yaml
scrape_configs:
  - job_name: "your-custom-service"
    static_configs:
      - targets: ["host.docker.internal:8080"]
```

## Dashboards

### EQ Service Dashboard
The main dashboard (`eq-service-dashboard.json`) includes:

- **Service Health**: Service uptime and availability
- **Request Metrics**: gRPC request rates and patterns
- **Job Processing**: Job success/failure rates and queue status
- **ZK Proof Performance**: Proof generation timing analysis
- **External Dependencies**: Celestia and Succinct network status
- **System Resources**: CPU, memory, and disk usage

### Importing Additional Dashboards

1. Open Grafana (http://localhost:3000)
2. Go to "+" → "Import"
3. Upload a JSON file or paste dashboard ID
4. Configure data source as "Prometheus"

## Alerting

### Alert Rules

The monitoring setup includes comprehensive alerting rules:

#### Service-Level Alerts
- **EqServiceDown**: Service is unreachable
- **EqServiceHighJobFailureRate**: >50% job failure rate
- **EqServiceSlowZkProofGeneration**: >5 min proof generation time
- **EqServiceJobsStuck**: Jobs not progressing

#### System-Level Alerts
- **EqServiceHighMemoryUsage**: >90% memory usage
- **EqServiceHighCpuUsage**: >80% CPU usage
- **EqServiceDiskSpaceLow**: <20% disk space remaining

#### External Dependencies
- **CelestiaNodeDown**: Celestia network unreachable
- **SuccinctNetworkDown**: Succinct network unreachable

### Notification Channels

#### Webhook Receiver
The included webhook receiver logs all alerts and provides different endpoints:

- `/webhook` - General alerts
- `/webhook/critical` - Critical alerts with special formatting
- `/webhook/eq-service` - EQ Service specific alerts
- `/webhook/eq-service/critical` - Critical EQ Service alerts
- `/webhook/external-deps` - External dependency alerts
- `/webhook/system` - System alerts

#### Email Notifications
To enable email notifications, edit `alertmanager/alertmanager.yml`:

```yaml
receivers:
  - name: 'email-notifications'
    email_configs:
    - to: 'team@example.com'
      from: 'alerts@eq-service.com'
      smarthost: 'smtp.gmail.com:587'
      auth_username: 'alerts@eq-service.com'
      auth_password: 'your-app-password'
      subject: 'EQ Service Alert: {{ .GroupLabels.alertname }}'
```

#### Slack Notifications
To enable Slack notifications:

```yaml
receivers:
  - name: 'slack-notifications'
    slack_configs:
    - api_url: 'https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK'
      channel: '#alerts'
      title: 'EQ Service Alert'
      text: '{{ range .Alerts }}{{ .Annotations.summary }}{{ end }}'
```

## Troubleshooting

### Common Issues

#### 1. EQ Service Not Found
```
Error: context deadline exceeded
```
**Solution**: Ensure EQ Service is running and accessible on port 9091.

#### 2. Permission Denied
```
Error: permission denied
```
**Solution**: Check Docker permissions and volume mounts.

#### 3. Out of Memory
```
Error: cannot allocate memory
```
**Solution**: Increase available memory or reduce retention time.

### Debug Commands

```bash
# Check all container logs
docker-compose logs

# Check specific service logs
docker-compose logs prometheus
docker-compose logs grafana
docker-compose logs alertmanager

# Check container resource usage
docker stats

# Test Prometheus targets
curl http://localhost:9090/api/v1/targets

# Test alert receiver
curl -X POST http://localhost:2021/webhook \
  -H "Content-Type: application/json" \
  -d '{"alerts": [{"status": "firing", "labels": {"alertname": "test"}}]}'
```

### Prometheus Configuration Validation

```bash
# Validate Prometheus config
docker run --rm -v $(pwd)/prometheus:/etc/prometheus \
  prom/prometheus:latest \
  promtool check config /etc/prometheus/prometheus.yml

# Validate alert rules
docker run --rm -v $(pwd)/prometheus:/etc/prometheus \
  prom/prometheus:latest \
  promtool check rules /etc/prometheus/alert_rules.yml
```

## Maintenance

### Data Retention

By default, metrics are retained for 200 hours. To change this:

1. Edit `docker-compose.yml`
2. Modify the `--storage.tsdb.retention.time` parameter
3. Restart Prometheus: `docker-compose restart prometheus`

### Backup

To backup your monitoring data:

```bash
# Backup Prometheus data
docker run --rm -v prometheus_data:/data -v $(pwd):/backup \
  busybox tar czf /backup/prometheus-backup.tar.gz /data

# Backup Grafana data
docker run --rm -v grafana_data:/data -v $(pwd):/backup \
  busybox tar czf /backup/grafana-backup.tar.gz /data
```

### Updates

To update the monitoring stack:

```bash
# Pull latest images
docker-compose pull

# Restart services
docker-compose up -d
```

## Production Considerations

### Security
- Change default passwords
- Enable TLS/SSL for external access
- Implement proper authentication
- Use secrets management for sensitive configuration

### Performance
- Configure appropriate retention policies
- Monitor resource usage
- Scale Prometheus for high-cardinality metrics
- Consider using remote storage for long-term retention

### High Availability
- Deploy multiple Prometheus instances
- Use Alertmanager clustering
- Implement load balancing for Grafana
- Regular backup procedures

## Support

For issues related to:
- **EQ Service metrics**: Check the EQ Service logs and metrics endpoint
- **Prometheus**: Check Prometheus logs and configuration
- **Grafana**: Check Grafana logs and dashboard configuration
- **Alertmanager**: Check Alertmanager logs and routing configuration

## References

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Alertmanager Documentation](https://prometheus.io/docs/alerting/alertmanager/)
- [EQ Service Documentation](../README.md)
