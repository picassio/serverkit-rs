"""Storage-cost estimation for backups.

Cost rates ($/GB/month) live in the existing backup config (``backups.json``)
under ``cost_rates``. ``local`` defaults to $0 (operator can set it if they track
disk cost); ``s3``/``b2`` carry sensible public list-price defaults. All money is
handled as :class:`~decimal.Decimal` to avoid float drift, and serialized to float
only at the API boundary.
"""
import math
from decimal import Decimal, ROUND_HALF_UP

GB = 1024 ** 3
_QUANT = Decimal('0.0001')

DEFAULT_RATES = {
    'local': 0.0,
    's3': 0.023,   # AWS S3 Standard, us-east-1 list price
    'b2': 0.006,   # Backblaze B2 list price
}


class BackupCostService:
    """Compute and project backup storage cost."""

    # ---- rates -----------------------------------------------------------

    @classmethod
    def get_rates(cls):
        """Return the effective rate table (defaults merged with config)."""
        rates = dict(DEFAULT_RATES)
        try:
            from app.services.backup_service import BackupService
            for key, value in (BackupService.get_config().get('cost_rates') or {}).items():
                try:
                    rates[key] = float(value)
                except (TypeError, ValueError):
                    continue
        except Exception:
            pass
        return rates

    @classmethod
    def save_rates(cls, rates):
        """Persist (merge) the provided rate overrides into the backup config."""
        from app.services.backup_service import BackupService
        config = BackupService.get_config()
        merged = dict(DEFAULT_RATES)
        merged.update(config.get('cost_rates') or {})
        for key, value in (rates or {}).items():
            if key not in DEFAULT_RATES:
                continue
            try:
                merged[key] = round(float(value), 6)
            except (TypeError, ValueError):
                continue
        config['cost_rates'] = merged
        BackupService.save_config(config)
        return merged

    @classmethod
    def get_rate(cls, provider):
        return cls.get_rates().get(provider, DEFAULT_RATES.get(provider, 0.0))

    @classmethod
    def configured_remote_provider(cls):
        """Which remote provider is active globally ('s3' | 'b2'), or None."""
        try:
            from app.services.storage_provider_service import StorageProviderService
            provider = (StorageProviderService.get_config() or {}).get('provider', 'local')
            return provider if provider and provider != 'local' else None
        except Exception:
            return None

    # ---- computation -----------------------------------------------------

    @classmethod
    def compute_cost(cls, size_bytes, provider):
        """Monthly storage cost (Decimal) for ``size_bytes`` on ``provider``."""
        rate = cls.get_rate(provider)
        gigabytes = Decimal(str((size_bytes or 0) / GB))
        return (gigabytes * Decimal(str(rate))).quantize(_QUANT, rounding=ROUND_HALF_UP)

    @classmethod
    def format_cost(cls, cost):
        """Human string like ``$0.04`` / ``$1.20`` / ``$0.0006`` for tiny values."""
        try:
            value = float(cost or 0)
        except (TypeError, ValueError):
            value = 0.0
        if value == 0 or value >= 0.01:
            return f"${value:.2f}"
        return f"${value:.4f}"

    @classmethod
    def runs_per_month(cls, cron):
        """Estimate how many times a cron fires in a 30-day window."""
        if cron:
            try:
                from croniter import croniter
                from datetime import datetime, timedelta
                if croniter.is_valid(cron):
                    base = datetime.utcnow()
                    end = base + timedelta(days=30)
                    itr = croniter(cron, base)
                    count = 0
                    while count < 2000:
                        nxt = itr.get_next(datetime)
                        if nxt > end:
                            break
                        count += 1
                    return count or 1
            except Exception:
                pass
        # Heuristic fallback: parse day-of-week / day-of-month fields.
        try:
            fields = (cron or '0 2 * * *').split()
            dow = fields[4] if len(fields) >= 5 else '*'
            if dow not in ('*', '?'):
                days = cls._count_dow(dow)
                return max(1, round(days * 30 / 7))
        except Exception:
            pass
        return 30  # assume daily

    @staticmethod
    def _count_dow(dow_field):
        seen = set()
        for part in dow_field.split(','):
            if '-' in part:
                try:
                    lo, hi = part.split('-')
                    for d in range(int(lo), int(hi) + 1):
                        seen.add(d % 7)
                except ValueError:
                    continue
            else:
                try:
                    seen.add(int(part) % 7)
                except ValueError:
                    continue
        return len(seen) or 1

    @classmethod
    def expected_retained_count(cls, policy):
        """How many backups a policy keeps in steady state (count ∧ days rules)."""
        per_month = cls.runs_per_month(policy.schedule_cron)
        per_day = per_month / 30.0
        by_days = math.ceil(per_day * (policy.retention_days or 1))
        return max(1, min(policy.retention_count or 1, by_days or 1))

    @classmethod
    def project_monthly_cost(cls, policy, avg_size):
        """Estimated monthly storage cost for one policy at ``avg_size`` per run."""
        retained = cls.expected_retained_count(policy)
        stored = (avg_size or 0) * retained
        cost = cls.compute_cost(stored, 'local')
        if policy.remote_copy:
            provider = cls.configured_remote_provider()
            if provider:
                cost += cls.compute_cost(stored, provider)
        return cost.quantize(_QUANT, rounding=ROUND_HALF_UP)
