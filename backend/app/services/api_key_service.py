"""Service for API key management."""
from datetime import datetime
from app import db
from app.models.api_key import ApiKey


class ApiKeyService:
    """CRUD and validation for API keys."""

    @staticmethod
    def create_key(user_id, name, scopes=None, tier='standard', expires_at=None):
        """Create a new API key. Returns (api_key_record, raw_key)."""
        if tier not in ApiKey.VALID_TIERS:
            tier = ApiKey.TIER_STANDARD

        raw_key, prefix, key_hash = ApiKey.generate_key()

        api_key = ApiKey(
            user_id=user_id,
            name=name,
            key_prefix=prefix,
            key_hash=key_hash,
            tier=tier,
            expires_at=expires_at,
        )
        api_key.set_scopes(scopes)

        db.session.add(api_key)
        db.session.commit()

        return api_key, raw_key

    @staticmethod
    def validate_key(raw_key):
        """Validate a raw API key. Returns ApiKey if valid, None otherwise."""
        if not raw_key or not raw_key.startswith('sk_'):
            return None

        key_hash = ApiKey.hash_key(raw_key)
        api_key = ApiKey.query.filter_by(key_hash=key_hash).first()

        if not api_key:
            return None
        if not api_key.is_valid():
            return None

        return api_key

    @staticmethod
    def revoke_key(key_id, user_id):
        """Revoke an API key."""
        api_key = ApiKey.query.filter_by(id=key_id, user_id=user_id).first()
        if not api_key:
            return None

        api_key.is_active = False
        api_key.revoked_at = datetime.utcnow()
        db.session.commit()
        return api_key

    @staticmethod
    def list_keys(user_id):
        """List all API keys for a user."""
        return ApiKey.query.filter_by(user_id=user_id).order_by(
            ApiKey.created_at.desc()
        ).all()

    @staticmethod
    def get_key(key_id, user_id=None):
        """Get a single API key by ID."""
        query = ApiKey.query.filter_by(id=key_id)
        if user_id:
            query = query.filter_by(user_id=user_id)
        return query.first()

    @staticmethod
    def update_key(key_id, user_id, name=None, scopes=None, tier=None):
        """Update an API key's metadata."""
        api_key = ApiKey.query.filter_by(id=key_id, user_id=user_id).first()
        if not api_key:
            return None

        if name is not None:
            api_key.name = name
        if scopes is not None:
            api_key.set_scopes(scopes)
        if tier is not None and tier in ApiKey.VALID_TIERS:
            api_key.tier = tier

        db.session.commit()
        return api_key

    @staticmethod
    def rotate_key(key_id, user_id):
        """Rotate an API key: revoke old, create new with same config."""
        old_key = ApiKey.query.filter_by(id=key_id, user_id=user_id).first()
        if not old_key:
            return None, None

        # Capture config before revoking
        name = old_key.name
        scopes = old_key.get_scopes()
        tier = old_key.tier
        expires_at = old_key.expires_at

        # Revoke old key
        old_key.is_active = False
        old_key.revoked_at = datetime.utcnow()

        # Create new key with same config
        new_key, raw_key = ApiKeyService.create_key(
            user_id=user_id,
            name=name,
            scopes=scopes,
            tier=tier,
            expires_at=expires_at,
        )

        return new_key, raw_key

    @staticmethod
    def check_scope(api_key, required_scope):
        """Check if an API key has the required scope."""
        if not api_key:
            return False
        return api_key.has_scope(required_scope)
