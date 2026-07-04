# Backend Dockerfile for ServerKit
#
# NOTE: A containerized backend CANNOT manage the host (no host Docker daemon,
# nginx, systemctl, or real /var/serverkit/apps). The canonical deployment runs
# the backend on the host via systemd (see install.sh + docker-compose.yml,
# which is frontend-only). This image is only for local/dev or all-in-one demos.
FROM python:3.11-slim-bookworm

# Set environment variables
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    FLASK_ENV=production \
    SERVERKIT_BACKEND_PORT=5000

# Set working directory
WORKDIR /app

# Install system dependencies including ClamAV for security scanning
RUN apt-get update && apt-get install -y --no-install-recommends \
    # Build dependencies
    gcc \
    python3-dev \
    libpq-dev \
    libffi-dev \
    libssl-dev \
    libevent-dev \
    # ClamAV antivirus
    clamav \
    clamav-daemon \
    clamav-freshclam \
    # Utilities
    curl \
    procps \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create directories for ServerKit
RUN mkdir -p /etc/serverkit /var/serverkit/apps /app/logs /var/quarantine /app/instance /var/backups/serverkit

# Copy requirements first for caching
COPY requirements.txt .

# Install Python dependencies
RUN pip install --no-cache-dir -r requirements.txt \
    && pip install --no-cache-dir gunicorn gevent gevent-websocket

# Copy application code
COPY . .

# Create non-root user and set permissions. /var/serverkit must be owned by
# appuser too, otherwise the (dev/demo) backend cannot create app directories
# under /var/serverkit/apps and fails with EACCES.
RUN useradd --create-home appuser \
    && chown -R appuser:appuser /app \
    && chown -R appuser:appuser /app/instance \
    && chown -R appuser:appuser /etc/serverkit \
    && chown -R appuser:appuser /var/serverkit \
    && chown -R appuser:appuser /var/quarantine \
    && chown -R appuser:appuser /var/backups/serverkit

USER appuser

# Expose port
EXPOSE 5000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f "http://localhost:${SERVERKIT_BACKEND_PORT:-5000}/api/v1/system/health" || exit 1

# Run with gunicorn + gevent for WebSocket support
CMD ["sh", "-c", "exec gunicorn -k geventwebsocket.gunicorn.workers.GeventWebSocketWorker -w 1 -b 0.0.0.0:${SERVERKIT_BACKEND_PORT:-5000} --timeout 120 --access-logfile - --error-logfile - run:app"]
