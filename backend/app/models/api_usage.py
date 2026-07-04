"""API usage tracking models."""
from datetime import datetime
from app import db


class ApiUsageLog(db.Model):
    """Raw API usage log for every request."""
    __tablename__ = 'api_usage_logs'

    id = db.Column(db.Integer, primary_key=True)
    api_key_id = db.Column(db.Integer, db.ForeignKey('api_keys.id'), nullable=True)
    user_id = db.Column(db.Integer, db.ForeignKey('users.id'), nullable=True)
    method = db.Column(db.String(10), nullable=False)
    endpoint = db.Column(db.String(500), nullable=False)
    blueprint = db.Column(db.String(100), nullable=True)
    status_code = db.Column(db.Integer, nullable=False)
    response_time_ms = db.Column(db.Float, nullable=True)
    ip_address = db.Column(db.String(45), nullable=True)
    user_agent = db.Column(db.String(500), nullable=True)
    request_size = db.Column(db.Integer, nullable=True)
    response_size = db.Column(db.Integer, nullable=True)
    created_at = db.Column(db.DateTime, default=datetime.utcnow, index=True)

    def to_dict(self):
        return {
            'id': self.id,
            'api_key_id': self.api_key_id,
            'user_id': self.user_id,
            'method': self.method,
            'endpoint': self.endpoint,
            'blueprint': self.blueprint,
            'status_code': self.status_code,
            'response_time_ms': self.response_time_ms,
            'ip_address': self.ip_address,
            'created_at': self.created_at.isoformat() if self.created_at else None,
        }


class ApiUsageSummary(db.Model):
    """Aggregated API usage summary per hour."""
    __tablename__ = 'api_usage_summaries'

    id = db.Column(db.Integer, primary_key=True)
    period_start = db.Column(db.DateTime, nullable=False, index=True)
    api_key_id = db.Column(db.Integer, db.ForeignKey('api_keys.id'), nullable=True)
    user_id = db.Column(db.Integer, db.ForeignKey('users.id'), nullable=True)
    endpoint = db.Column(db.String(500), nullable=True)
    total_requests = db.Column(db.Integer, default=0)
    success_count = db.Column(db.Integer, default=0)
    client_error_count = db.Column(db.Integer, default=0)
    server_error_count = db.Column(db.Integer, default=0)
    avg_response_time_ms = db.Column(db.Float, nullable=True)
    max_response_time_ms = db.Column(db.Float, nullable=True)

    def to_dict(self):
        return {
            'id': self.id,
            'period_start': self.period_start.isoformat() if self.period_start else None,
            'api_key_id': self.api_key_id,
            'user_id': self.user_id,
            'endpoint': self.endpoint,
            'total_requests': self.total_requests,
            'success_count': self.success_count,
            'client_error_count': self.client_error_count,
            'server_error_count': self.server_error_count,
            'avg_response_time_ms': self.avg_response_time_ms,
            'max_response_time_ms': self.max_response_time_ms,
        }
