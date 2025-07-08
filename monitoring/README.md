# EQ Service Monitoring Infrastructure

This directory contains a comprehensive monitoring setup for the EQ Service using Prometheus, Grafana, and Alertmanager. The setup is based on Docker Compose and provides full observability and alerting for the EQ Service application.

## Architecture

With default ports:

```
┌─────────────────┐    ┌─────────────────┐
│   Blackbox Exp. │    │ Node Exporter   │
│   (Port 9115)   │    │   (Port 9100)   │
└─────────────────┘    └─────────────────┘
         │                      │
         └────────────────▼     ▼
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
```

## Components

### Core Monitoring Stack

- **Prometheus**: Time-series database and monitoring system
- **Grafana**: Visualization and dashboards
- **Alertmanager**: Alert routing and notifications
- **Node Exporter**: System metrics collection
- **cAdvisor**: Container metrics collection
- **Blackbox Exporter**: Upstream endpoint monitoring
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

#### Upstream Dependencies

- Celestia node connectivity
- Succinct ZK prover network connectivity

## Quick Start

### Prerequisites

1. Docker installed (with compose v2 support)
2. EQ Service running and exposing metrics on port 9091
3. At least 4GB RAM available for the monitoring stack

### Configuration

#### EQ Service

Ensure your EQ Service is exposing metrics on port set in `../.env` = `EQ_PROMETHEUS_SOCKET=127.0.0.1:9091` and `EQ_PROMETHEUS_PORT=9091` by default.
The monitoring stack expects the service to be accessible at `host.docker.internal:${EQ_PROMETHEUS_PORT}` from within the containers.

You can customize the monitoring setup using environment variables in `../.env`, see [../example.env](../example.env) for details.

#### Configuration Testing

To test and validate your monitoring configuration, use [./test-config.sh](./test-config.sh):

```sh
# Get help
./test-config.sh --help

# Test all configurations
./test-config.sh

# Show configuration summary
./test-config.sh --summary
```

The test script validates:

- All required configuration files are present
- Docker Compose configuration is valid
- Port configurations are within valid ranges
- Grafana environment variables are properly set
- Prometheus retention format is correct
- Receiver configuration is valid
- Service dependencies are correct

The system uses static configuration with Docker Compose environment variable substitution for port mappings.

#### Management Tooling

To start and interact with all monitoring services, use [./monitoring-tools.sh](./monitoring-tools.sh):

```sh
# More info on script
./monitoring-tools.sh --help

# Start monitoring stack
./monitoring-tools.sh

# Validate configurations only
./monitoring-tools.sh --validate

# Check service status
./monitoring-tools.sh --status

# Stop all services
./monitoring-tools.sh --stop

# Restart services
./monitoring-tools.sh --restart

# View logs
./monitoring-tools.sh --logs

# Follow logs in real-time
./monitoring-tools.sh --logs-follow

# Check EQ Service connectivity
./monitoring-tools.sh --check-eq
```

### Access the Services

**Note**: The actual ports will be determined by the environment variables set in `../.env`. The values shown above are defaults if no environment variables are set.

- **Grafana**: <http://localhost:3000>
  - Dashboard: <http://localhost:3000/d/eq-service-dashboard/eq-service-dashboard>
  - Set credentials in `../.env` = `GF_SECURITY_ADMIN_USER=admin` and `GF_SECURITY_ADMIN_PASSWORD=admin` by default
- **Prometheus**: <http://localhost:9090>
- **Alertmanager**: <http://localhost:9093>
- **Alert Receiver**: <http://localhost:2021>

## Configuration Approach

The monitoring stack uses a **clean static configuration approach** with Docker Compose environment variable substitution:

### How It Works

1. **Internal Services**: Use Docker service names with fixed internal ports
   - `prometheus:9090`, `grafana:3000`, `node-exporter:9100`, etc.
   - These ports are not exposed externally unless mapped

2. **External Port Mapping**: Configurable via environment variables
   - `PROMETHEUS_PORT=9090` maps to `prometheus:9090`
   - `GRAFANA_PORT=3000` maps to `grafana:3000`
   - Docker Compose handles the port mapping: `${PROMETHEUS_PORT:-9090}:9090`

3. **External Services**: Use static default ports in configuration
   - **EQ Service**: `host.docker.internal:9091`
   - **Celestia Node**: `host.docker.internal:26658`
   - If you change these ports, update your EQ Service and Celestia Node configuration

### Benefits

- **No templates or complex processing** - Pure static configuration
- **Docker Compose handles substitution** - Clean and standard approach
- **Minimal complexity** - Easy to understand and maintain
- **Fast startup** - No configuration processing delays

## Dashboards

### EQ Service Dashboard

The main dashboard (`eq-service-dashboard.json`) includes:

- **Service Health**: Service uptime and availability
- **Request Metrics**: gRPC request rates and patterns
- **Job Processing**: Job success/failure rates and queue status
- **ZK Proof Performance**: Proof generation timing analysis
- **Upstream Dependencies**: Celestia and Succinct network status
- **System Resources**: CPU, memory, and disk usage

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

#### Upstream Dependencies

- **CelestiaNodeDown**: Celestia network unreachable
- **SuccinctNetworkDown**: Succinct network unreachable

### Notification Channels

#### Webhook Receiver

The included webhook receiver logs all alerts and provides different endpoints:

- `/webhook` - General alerts
- `/webhook/critical` - Critical alerts with special formatting
- `/webhook/eq-service` - EQ Service specific alerts
- `/webhook/eq-service/critical` - Critical EQ Service alerts
- `/webhook/external-deps` - Upstream dependency alerts
- `/webhook/system` - System alerts

#### Email Notifications

To enable email notifications, edit `alertmanager/alertmanager.yml`:

```yaml
receivers:
  - name: "email-notifications"
    email_configs:
      - to: "team@example.com"
        from: "alerts@eq-service.com"
        smarthost: "smtp.gmail.com:587"
        auth_username: "alerts@eq-service.com"
        auth_password: "your-app-password"
        subject: "EQ Service Alert: {{ .GroupLabels.alertname }}"
```

#### Slack Notifications

To enable Slack notifications:

```yaml
receivers:
  - name: "slack-notifications"
    slack_configs:
      - api_url: "https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK"
        channel: "#alerts"
        title: "EQ Service Alert"
        text: "{{ range .Alerts }}{{ .Annotations.summary }}{{ end }}"
```

## Debug Commands

````sh
# Check all container logs
docker compose logs

# Check specific service logs
docker compose logs prometheus
docker compose logs grafana
docker compose logs alertmanager

```sh
# Check container resource usage
docker stats

# Test Prometheus targets (using environment variable or default port)
curl http://localhost:${PROMETHEUS_PORT:-9090}/api/v1/targets

# Test alert receiver (using environment variable or default port)
curl -X POST http://localhost:${RECEIVER_PORT:-2021}/webhook \
  -H "Content-Type: application/json" \
  -d '{"alerts": [{"status": "firing", "labels": {"alertname": "test"}}]}'
````

## Maintenance

### Data Retention

By default, metrics are retained for 200 hours. To change this:

1. Edit `docker-compose.yml`
2. Modify the `--storage.tsdb.retention.time` parameter
3. Restart Prometheus: `docker compose restart prometheus`

### Backup

To backup your monitoring data:

```sh
# Backup Prometheus data
docker run --rm -v prometheus_data:/data -v $(pwd):/backup \
  busybox tar czf /backup/prometheus-backup.tar.gz /data

# Backup Grafana data
docker run --rm -v grafana_data:/data -v $(pwd):/backup \
  busybox tar czf /backup/grafana-backup.tar.gz /data
```

### Updates

To update the monitoring stack:

```sh
# Pull latest images
docker compose pull

# Restart services
docker compose up -d
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

## Complete Example

Here's a complete example of setting up the monitoring stack with custom ports:

### 1. Configure Environment Variables

Edit your `../.env` file:

```sh
# Monitoring Service Ports (External Access)
PROMETHEUS_PORT=9090
ALERTMANAGER_PORT=9093
GRAFANA_PORT=3000
NODE_EXPORTER_PORT=9100
CADVISOR_PORT=8080
BLACKBOX_EXPORTER_PORT=9115

# Grafana Configuration
GF_SECURITY_ADMIN_USER=admin
GF_SECURITY_ADMIN_PASSWORD=secure_password_here
GF_USERS_ALLOW_SIGN_UP=false
GF_INSTALL_PLUGINS=grafana-clock-panel,grafana-simple-json-datasource

# Alert Receiver Configuration
RECEIVER_DEBUG=false
RECEIVER_PORT=2021

# Prometheus Configuration
PROMETHEUS_RETENTION=200h
```

**Note**: The system uses static configuration with Docker Compose port mapping. External services (EQ Service on port 9091, Celestia Node on port 26658) are configured with fixed ports in the prometheus.yml file. The variables above control external access to the monitoring services.

### 2. Test Configuration

```sh
# Test all configurations
./test-config.sh --all

# Show configuration summary
./test-config.sh --summary
```

### 3. Start the Monitoring Stack

```sh
# Start all services
./monitoring-tools.sh

# Check service status
./monitoring-tools.sh --status

# Check EQ Service connectivity
./monitoring-tools.sh --check-eq
```

### 4. Access Services

- **Grafana**: http://localhost:3000 (admin/secure_password_here)
- **Prometheus**: http://localhost:9090
- **Alertmanager**: http://localhost:9093
- **Alert Receiver**: http://localhost:2021

## References

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Alertmanager Documentation](https://prometheus.io/docs/alerting/alertmanager/)
- [EQ Service Documentation](../README.md)
