"""Service for API usage analytics."""
from datetime import datetime, timedelta
from sqlalchemy import func, case
from app import db
from app.models.api_usage import ApiUsageLog, ApiUsageSummary


class ApiAnalyticsService:
    """Query and aggregate API usage data."""

    @staticmethod
    def get_overview(period='24h'):
        """Get overall API usage stats for a period."""
        since = _period_to_datetime(period)
        query = ApiUsageLog.query.filter(ApiUsageLog.created_at >= since)

        total = query.count()
        if total == 0:
            return {
                'total_requests': 0,
                'error_rate': 0,
                'avg_response_time_ms': 0,
                'success_count': 0,
                'client_error_count': 0,
                'server_error_count': 0,
            }

        stats = db.session.query(
            func.count(ApiUsageLog.id).label('total'),
            func.avg(ApiUsageLog.response_time_ms).label('avg_time'),
            func.sum(case((ApiUsageLog.status_code < 400, 1), else_=0)).label('success'),
            func.sum(case((ApiUsageLog.status_code.between(400, 499), 1), else_=0)).label('client_errors'),
            func.sum(case((ApiUsageLog.status_code >= 500, 1), else_=0)).label('server_errors'),
        ).filter(ApiUsageLog.created_at >= since).first()

        total_req = stats.total or 0
        error_count = (stats.client_errors or 0) + (stats.server_errors or 0)

        return {
            'total_requests': total_req,
            'error_rate': round(error_count / total_req * 100, 2) if total_req > 0 else 0,
            'avg_response_time_ms': round(stats.avg_time or 0, 2),
            'success_count': stats.success or 0,
            'client_error_count': stats.client_errors or 0,
            'server_error_count': stats.server_errors or 0,
        }

    @staticmethod
    def get_endpoint_stats(period='24h', limit=20):
        """Get top endpoints by request count."""
        since = _period_to_datetime(period)

        results = db.session.query(
            ApiUsageLog.endpoint,
            ApiUsageLog.method,
            func.count(ApiUsageLog.id).label('count'),
            func.avg(ApiUsageLog.response_time_ms).label('avg_time'),
            func.sum(case((ApiUsageLog.status_code >= 400, 1), else_=0)).label('errors'),
        ).filter(
            ApiUsageLog.created_at >= since
        ).group_by(
            ApiUsageLog.endpoint, ApiUsageLog.method
        ).order_by(
            func.count(ApiUsageLog.id).desc()
        ).limit(limit).all()

        return [{
            'endpoint': r.endpoint,
            'method': r.method,
            'count': r.count,
            'avg_response_time_ms': round(r.avg_time or 0, 2),
            'error_count': r.errors or 0,
        } for r in results]

    @staticmethod
    def get_error_stats(period='24h'):
        """Get error breakdown by status code."""
        since = _period_to_datetime(period)

        results = db.session.query(
            ApiUsageLog.status_code,
            ApiUsageLog.endpoint,
            func.count(ApiUsageLog.id).label('count'),
        ).filter(
            ApiUsageLog.created_at >= since,
            ApiUsageLog.status_code >= 400,
        ).group_by(
            ApiUsageLog.status_code, ApiUsageLog.endpoint
        ).order_by(
            func.count(ApiUsageLog.id).desc()
        ).limit(50).all()

        return [{
            'status_code': r.status_code,
            'endpoint': r.endpoint,
            'count': r.count,
        } for r in results]

    @staticmethod
    def get_time_series(period='24h', interval='hour'):
        """Get request counts over time for charting."""
        since = _period_to_datetime(period)

        # Use raw logs grouped by truncated time
        if interval == 'hour':
            trunc = func.strftime('%Y-%m-%d %H:00:00', ApiUsageLog.created_at)
        elif interval == 'day':
            trunc = func.strftime('%Y-%m-%d', ApiUsageLog.created_at)
        else:
            trunc = func.strftime('%Y-%m-%d %H:%M:00', ApiUsageLog.created_at)

        results = db.session.query(
            trunc.label('period'),
            func.count(ApiUsageLog.id).label('count'),
            func.sum(case((ApiUsageLog.status_code >= 400, 1), else_=0)).label('errors'),
            func.avg(ApiUsageLog.response_time_ms).label('avg_time'),
        ).filter(
            ApiUsageLog.created_at >= since
        ).group_by('period').order_by('period').all()

        return [{
            'period': r.period,
            'count': r.count,
            'errors': r.errors or 0,
            'avg_response_time_ms': round(r.avg_time or 0, 2),
        } for r in results]

    @staticmethod
    def get_key_usage(api_key_id, period='24h'):
        """Get usage stats for a specific API key."""
        since = _period_to_datetime(period)

        stats = db.session.query(
            func.count(ApiUsageLog.id).label('total'),
            func.avg(ApiUsageLog.response_time_ms).label('avg_time'),
            func.sum(case((ApiUsageLog.status_code >= 400, 1), else_=0)).label('errors'),
        ).filter(
            ApiUsageLog.api_key_id == api_key_id,
            ApiUsageLog.created_at >= since,
        ).first()

        # Top endpoints for this key
        endpoints = db.session.query(
            ApiUsageLog.endpoint,
            func.count(ApiUsageLog.id).label('count'),
        ).filter(
            ApiUsageLog.api_key_id == api_key_id,
            ApiUsageLog.created_at >= since,
        ).group_by(
            ApiUsageLog.endpoint
        ).order_by(
            func.count(ApiUsageLog.id).desc()
        ).limit(10).all()

        return {
            'total_requests': stats.total or 0,
            'avg_response_time_ms': round(stats.avg_time or 0, 2),
            'error_count': stats.errors or 0,
            'top_endpoints': [{'endpoint': e.endpoint, 'count': e.count} for e in endpoints],
        }

    @staticmethod
    def aggregate_hourly():
        """Roll up raw logs into hourly summaries."""
        # Find the latest summary period
        latest = db.session.query(
            func.max(ApiUsageSummary.period_start)
        ).scalar()

        if latest:
            since = latest
        else:
            since = datetime.utcnow() - timedelta(days=7)

        # Group raw logs by hour, endpoint, user, api_key
        trunc = func.strftime('%Y-%m-%d %H:00:00', ApiUsageLog.created_at)

        results = db.session.query(
            trunc.label('period'),
            ApiUsageLog.api_key_id,
            ApiUsageLog.user_id,
            ApiUsageLog.endpoint,
            func.count(ApiUsageLog.id).label('total'),
            func.sum(case((ApiUsageLog.status_code < 400, 1), else_=0)).label('success'),
            func.sum(case((ApiUsageLog.status_code.between(400, 499), 1), else_=0)).label('client_errors'),
            func.sum(case((ApiUsageLog.status_code >= 500, 1), else_=0)).label('server_errors'),
            func.avg(ApiUsageLog.response_time_ms).label('avg_time'),
            func.max(ApiUsageLog.response_time_ms).label('max_time'),
        ).filter(
            ApiUsageLog.created_at >= since
        ).group_by(
            'period', ApiUsageLog.api_key_id, ApiUsageLog.user_id, ApiUsageLog.endpoint
        ).all()

        for r in results:
            try:
                period_start = datetime.strptime(r.period, '%Y-%m-%d %H:%M:%S')
            except (ValueError, TypeError):
                continue

            # Check if summary already exists
            existing = ApiUsageSummary.query.filter_by(
                period_start=period_start,
                api_key_id=r.api_key_id,
                user_id=r.user_id,
                endpoint=r.endpoint,
            ).first()

            if existing:
                continue

            summary = ApiUsageSummary(
                period_start=period_start,
                api_key_id=r.api_key_id,
                user_id=r.user_id,
                endpoint=r.endpoint,
                total_requests=r.total or 0,
                success_count=r.success or 0,
                client_error_count=r.client_errors or 0,
                server_error_count=r.server_errors or 0,
                avg_response_time_ms=round(r.avg_time or 0, 2),
                max_response_time_ms=round(r.max_time or 0, 2),
            )
            db.session.add(summary)

        db.session.commit()

    @staticmethod
    def cleanup_old_logs(days=30):
        """Purge raw usage logs older than specified days."""
        cutoff = datetime.utcnow() - timedelta(days=days)
        deleted = ApiUsageLog.query.filter(ApiUsageLog.created_at < cutoff).delete()
        db.session.commit()
        return deleted


def _period_to_datetime(period):
    """Convert a period string to a datetime."""
    now = datetime.utcnow()
    if period == '1h':
        return now - timedelta(hours=1)
    elif period == '24h':
        return now - timedelta(hours=24)
    elif period == '7d':
        return now - timedelta(days=7)
    elif period == '30d':
        return now - timedelta(days=30)
    elif period == '90d':
        return now - timedelta(days=90)
    return now - timedelta(hours=24)
