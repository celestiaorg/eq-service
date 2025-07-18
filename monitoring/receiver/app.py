#!/usr/bin/env python3
"""
Simple webhook receiver for Alertmanager notifications.
This service receives webhook notifications from Alertmanager and logs them.
"""

import json
import logging
from datetime import datetime
from flask import Flask, request, jsonify
import os

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

app = Flask(__name__)

def format_alert(alert):
    """Format an alert for logging."""
    status = alert.get('status', 'unknown')
    labels = alert.get('labels', {})
    annotations = alert.get('annotations', {})

    alert_name = labels.get('alertname', 'Unknown')
    severity = labels.get('severity', 'unknown')
    instance = labels.get('instance', 'unknown')

    summary = annotations.get('summary', 'No summary available')
    description = annotations.get('description', 'No description available')

    starts_at = alert.get('startsAt', '')
    ends_at = alert.get('endsAt', '')

    return {
        'alert_name': alert_name,
        'severity': severity,
        'instance': instance,
        'status': status,
        'summary': summary,
        'description': description,
        'starts_at': starts_at,
        'ends_at': ends_at,
        'labels': labels,
        'annotations': annotations
    }

@app.route('/webhook', methods=['POST'])
def webhook():
    """Handle general webhook alerts."""
    try:
        data = request.get_json()
        if not data:
            logger.warning("Received empty webhook payload")
            return jsonify({'status': 'error', 'message': 'Empty payload'}), 400

        alerts = data.get('alerts', [])
        group_labels = data.get('groupLabels', {})

        logger.info(f"Received webhook with {len(alerts)} alerts, group labels: {group_labels}")

        for alert in alerts:
            formatted_alert = format_alert(alert)
            logger.info(f"ALERT: {formatted_alert['alert_name']} - {formatted_alert['severity']} - {formatted_alert['summary']}")
            logger.info(f"  Instance: {formatted_alert['instance']}")
            logger.info(f"  Status: {formatted_alert['status']}")
            logger.info(f"  Description: {formatted_alert['description']}")

            if formatted_alert['status'] == 'resolved':
                logger.info(f"  ✅ Alert resolved at: {formatted_alert['ends_at']}")
            else:
                logger.info(f"  🔥 Alert started at: {formatted_alert['starts_at']}")

        return jsonify({'status': 'success', 'processed': len(alerts)}), 200

    except Exception as e:
        logger.error(f"Error processing webhook: {str(e)}")
        return jsonify({'status': 'error', 'message': str(e)}), 500

@app.route('/webhook/critical', methods=['POST'])
def webhook_critical():
    """Handle critical alerts with special formatting."""
    try:
        data = request.get_json()
        if not data:
            logger.warning("Received empty critical webhook payload")
            return jsonify({'status': 'error', 'message': 'Empty payload'}), 400

        alerts = data.get('alerts', [])

        logger.critical(f"🚨 CRITICAL ALERT RECEIVED - {len(alerts)} alerts")

        for alert in alerts:
            formatted_alert = format_alert(alert)
            logger.critical(f"🚨 CRITICAL: {formatted_alert['alert_name']}")
            logger.critical(f"  Summary: {formatted_alert['summary']}")
            logger.critical(f"  Description: {formatted_alert['description']}")
            logger.critical(f"  Instance: {formatted_alert['instance']}")
            logger.critical(f"  Status: {formatted_alert['status']}")

            # Additional logging for critical alerts
            print(f"\n{'='*60}")
            print(f"CRITICAL ALERT: {formatted_alert['alert_name']}")
            print(f"Time: {datetime.now().isoformat()}")
            print(f"Summary: {formatted_alert['summary']}")
            print(f"Description: {formatted_alert['description']}")
            print(f"Instance: {formatted_alert['instance']}")
            print(f"Severity: {formatted_alert['severity']}")
            print(f"Status: {formatted_alert['status']}")
            print(f"{'='*60}\n")

        return jsonify({'status': 'success', 'processed': len(alerts)}), 200

    except Exception as e:
        logger.error(f"Error processing critical webhook: {str(e)}")
        return jsonify({'status': 'error', 'message': str(e)}), 500

@app.route('/webhook/eq-service', methods=['POST'])
def webhook_eq_service():
    """Handle EQ Service specific alerts."""
    try:
        data = request.get_json()
        if not data:
            logger.warning("Received empty EQ Service webhook payload")
            return jsonify({'status': 'error', 'message': 'Empty payload'}), 400

        alerts = data.get('alerts', [])

        logger.info(f"📊 EQ SERVICE ALERT - {len(alerts)} alerts")

        for alert in alerts:
            formatted_alert = format_alert(alert)
            logger.info(f"📊 EQ-SERVICE: {formatted_alert['alert_name']}")
            logger.info(f"  Summary: {formatted_alert['summary']}")
            logger.info(f"  Description: {formatted_alert['description']}")
            logger.info(f"  Severity: {formatted_alert['severity']}")
            logger.info(f"  Status: {formatted_alert['status']}")

        return jsonify({'status': 'success', 'processed': len(alerts)}), 200

    except Exception as e:
        logger.error(f"Error processing EQ Service webhook: {str(e)}")
        return jsonify({'status': 'error', 'message': str(e)}), 500

@app.route('/webhook/eq-service/critical', methods=['POST'])
def webhook_eq_service_critical():
    """Handle critical EQ Service alerts."""
    try:
        data = request.get_json()
        if not data:
            logger.warning("Received empty critical EQ Service webhook payload")
            return jsonify({'status': 'error', 'message': 'Empty payload'}), 400

        alerts = data.get('alerts', [])

        logger.critical(f"🚨📊 CRITICAL EQ SERVICE ALERT - {len(alerts)} alerts")

        for alert in alerts:
            formatted_alert = format_alert(alert)
            logger.critical(f"🚨📊 CRITICAL EQ-SERVICE: {formatted_alert['alert_name']}")
            logger.critical(f"  Summary: {formatted_alert['summary']}")
            logger.critical(f"  Description: {formatted_alert['description']}")
            logger.critical(f"  Instance: {formatted_alert['instance']}")
            logger.critical(f"  Status: {formatted_alert['status']}")

            # Special handling for EQ Service critical alerts
            if 'EqServiceDown' in formatted_alert['alert_name']:
                logger.critical("🚨 EQ SERVICE IS DOWN - IMMEDIATE ATTENTION REQUIRED")
            elif 'HighJobFailureRate' in formatted_alert['alert_name']:
                logger.critical("🚨 HIGH JOB FAILURE RATE - CHECK ZK PROVER AND CELESTIA CONNECTIONS")
            elif 'VerySlowZkProofGeneration' in formatted_alert['alert_name']:
                logger.critical("🚨 ZK PROOF GENERATION VERY SLOW - CHECK SUCCINCT NETWORK STATUS")

        return jsonify({'status': 'success', 'processed': len(alerts)}), 200

    except Exception as e:
        logger.error(f"Error processing critical EQ Service webhook: {str(e)}")
        return jsonify({'status': 'error', 'message': str(e)}), 500

@app.route('/webhook/external-deps', methods=['POST'])
def webhook_external_deps():
    """Handle external dependency alerts."""
    try:
        data = request.get_json()
        if not data:
            logger.warning("Received empty external deps webhook payload")
            return jsonify({'status': 'error', 'message': 'Empty payload'}), 400

        alerts = data.get('alerts', [])

        logger.warning(f"🔗 EXTERNAL DEPENDENCY ALERT - {len(alerts)} alerts")

        for alert in alerts:
            formatted_alert = format_alert(alert)
            logger.warning(f"🔗 EXTERNAL-DEPS: {formatted_alert['alert_name']}")
            logger.warning(f"  Summary: {formatted_alert['summary']}")
            logger.warning(f"  Description: {formatted_alert['description']}")
            logger.warning(f"  Status: {formatted_alert['status']}")

        return jsonify({'status': 'success', 'processed': len(alerts)}), 200

    except Exception as e:
        logger.error(f"Error processing external deps webhook: {str(e)}")
        return jsonify({'status': 'error', 'message': str(e)}), 500

@app.route('/webhook/system', methods=['POST'])
def webhook_system():
    """Handle system alerts."""
    try:
        data = request.get_json()
        if not data:
            logger.warning("Received empty system webhook payload")
            return jsonify({'status': 'error', 'message': 'Empty payload'}), 400

        alerts = data.get('alerts', [])

        logger.info(f"🖥️ SYSTEM ALERT - {len(alerts)} alerts")

        for alert in alerts:
            formatted_alert = format_alert(alert)
            logger.info(f"🖥️ SYSTEM: {formatted_alert['alert_name']}")
            logger.info(f"  Summary: {formatted_alert['summary']}")
            logger.info(f"  Description: {formatted_alert['description']}")
            logger.info(f"  Status: {formatted_alert['status']}")

        return jsonify({'status': 'success', 'processed': len(alerts)}), 200

    except Exception as e:
        logger.error(f"Error processing system webhook: {str(e)}")
        return jsonify({'status': 'error', 'message': str(e)}), 500

@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint."""
    return jsonify({'status': 'healthy', 'timestamp': datetime.now().isoformat()}), 200

@app.route('/', methods=['GET'])
def index():
    """Root endpoint with basic information."""
    return jsonify({
        'service': 'EQ Service Alert Receiver',
        'version': '1.0.0',
        'endpoints': [
            '/webhook',
            '/webhook/critical',
            '/webhook/eq-service',
            '/webhook/eq-service/critical',
            '/webhook/external-deps',
            '/webhook/system',
            '/health'
        ]
    }), 200

if __name__ == '__main__':
    port = int(os.environ.get('PORT', 2021))
    debug = os.environ.get('DEBUG', 'false').lower() == 'true'

    logger.info(f"Starting EQ Service Alert Receiver on port {port}")
    logger.info(f"Debug mode: {debug}")

    app.run(host='0.0.0.0', port=port, debug=debug)
