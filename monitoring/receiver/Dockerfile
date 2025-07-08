FROM python:3.9-slim

WORKDIR /app

# Install required packages
COPY requirements.txt .
RUN pip install -r requirements.txt

# Copy the application
COPY app.py .

# Create a non-root user
RUN useradd -r -s /bin/false receiver
USER receiver

# Expose port (default 2021, configurable via environment)
EXPOSE 2021

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:2021/health || exit 1

# Run the application
CMD ["python", "app.py"]
