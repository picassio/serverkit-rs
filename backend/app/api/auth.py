import logging
from datetime import datetime
from flask import Blueprint, request, jsonify
from sqlalchemy import func
from flask_jwt_extended import (
    create_access_token,
    create_refresh_token,
    jwt_required,
    get_jwt_identity,
    get_jwt
)
from app import db, limiter
from app.models import User, AuditLog, SystemSettings
from app.services.settings_service import SettingsService
from app.services.audit_service import AuditService

logger = logging.getLogger(__name__)

auth_bp = Blueprint('auth', __name__)


@auth_bp.route('/setup-status', methods=['GET'])
def get_setup_status():
    """Check if initial setup is needed and if registration is enabled."""
    needs_setup = SettingsService.needs_setup()
    registration_enabled = SettingsService.is_registration_enabled()

    # SSO info for login page
    from app.services import sso_service
    sso_providers = sso_service.get_enabled_providers()
    password_login_enabled = sso_service.is_password_login_allowed()

    # Migration status
    from app.services.migration_service import MigrationService
    migration_status = MigrationService.get_status()

    return jsonify({
        'needs_setup': needs_setup,
        'registration_enabled': registration_enabled,
        'sso_providers': sso_providers,
        'password_login_enabled': password_login_enabled,
        'needs_migration': migration_status['needs_migration'],
        'migration_info': {
            'pending_count': migration_status['pending_count'],
            'current_revision': migration_status['current_revision'],
            'head_revision': migration_status['head_revision'],
        },
    }), 200


@auth_bp.route('/register', methods=['POST'])
@limiter.limit("3 per minute")
def register():
    data = request.get_json()

    if not data:
        return jsonify({'error': 'No data provided'}), 400

    invite_token = data.get('invite_token')
    invitation = None

    # Validate invitation if provided
    if invite_token:
        from app.services.invitation_service import InvitationService
        invitation = InvitationService.validate_token(invite_token)
        if not invitation:
            return jsonify({'error': 'Invalid or expired invitation'}), 400

    # Check if registration is allowed
    is_first_user = User.query.count() == 0
    if not is_first_user and not invitation:
        if not SettingsService.needs_setup() and not SettingsService.is_registration_enabled():
            logger.warning(f"Registration attempt blocked - setup already completed. IP: {request.remote_addr}")
            return jsonify({'error': 'Registration is disabled'}), 403
        if not SettingsService.is_registration_enabled():
            return jsonify({'error': 'Registration is disabled'}), 403

    email = data.get('email')
    username = data.get('username')
    password = data.get('password')

    if not all([email, username, password]):
        return jsonify({'error': 'Missing required fields'}), 400

    if User.query.filter(func.lower(User.email) == func.lower(email)).first():
        return jsonify({'error': 'This email or username is unavailable'}), 409

    if User.query.filter_by(username=username).first():
        return jsonify({'error': 'This email or username is unavailable'}), 409

    if len(password) < 8:
        return jsonify({'error': 'Password must be at least 8 characters'}), 400

    # Determine role and permissions from invitation or defaults
    if is_first_user:
        role = User.ROLE_ADMIN
    elif invitation:
        role = invitation.role
    else:
        role = User.ROLE_DEVELOPER

    user = User(
        email=email,
        username=username,
        role=role
    )
    user.set_password(password)

    # Apply custom permissions from invitation
    if invitation and invitation.get_permissions():
        user.set_permissions(invitation.get_permissions())

    db.session.add(user)
    db.session.commit()

    # Mark invitation accepted
    if invitation:
        from app.services.invitation_service import InvitationService
        InvitationService.accept_invitation(invite_token, user.id)

        AuditService.log(
            action=AuditLog.ACTION_INVITATION_ACCEPT,
            user_id=user.id,
            target_type='invitation',
            target_id=invitation.id,
            details={'role': role}
        )

    # Log the user creation
    AuditService.log_user_action(
        action=AuditLog.ACTION_USER_CREATE,
        user_id=user.id,
        target_user_id=user.id,
        details={
            'username': username,
            'role': user.role,
            'self_registration': True,
            'via_invitation': invitation is not None
        }
    )
    db.session.commit()

    access_token = create_access_token(identity=user.id)
    refresh_token = create_refresh_token(identity=user.id)

    return jsonify({
        'message': 'User registered successfully',
        'user': user.to_dict(),
        'access_token': access_token,
        'refresh_token': refresh_token,
        'is_first_user': is_first_user
    }), 201


ALLOWED_USE_CASES = {'wordpress', 'web-apps', 'self-hosted', 'devops'}


@auth_bp.route('/complete-onboarding', methods=['POST'])
@jwt_required()
def complete_onboarding():
    """Complete the onboarding wizard and mark setup as done."""
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)

    if not user or user.role != User.ROLE_ADMIN:
        return jsonify({'error': 'Admin access required'}), 403

    data = request.get_json() or {}
    use_cases = data.get('use_cases', [])

    # Validate use_cases
    if not isinstance(use_cases, list):
        return jsonify({'error': 'use_cases must be a list'}), 400

    invalid = set(use_cases) - ALLOWED_USE_CASES
    if invalid:
        return jsonify({'error': f'Invalid use cases: {", ".join(invalid)}'}), 400

    # Save onboarding use cases
    SettingsService.set('onboarding_use_cases', use_cases, user_id=user.id)

    # Mark setup as complete
    SettingsService.complete_setup(user_id=user.id)

    return jsonify({'message': 'Onboarding completed successfully'}), 200


@auth_bp.route('/login', methods=['POST'])
@limiter.limit("5 per minute")
def login():
    # Check if password login is disabled (SSO-only mode)
    from app.services import sso_service
    if not sso_service.is_password_login_allowed():
        return jsonify({'error': 'Password login is disabled. Please use SSO.'}), 403

    data = request.get_json()

    if not data:
        return jsonify({'error': 'No data provided'}), 400

    login_id = data.get('email')  # Can be email or username
    password = data.get('password')

    if not all([login_id, password]):
        return jsonify({'error': 'Missing email/username or password'}), 400

    # Try to find user by email (case-insensitive) or username
    user = User.query.filter(
        (func.lower(User.email) == func.lower(login_id)) | (User.username == login_id)
    ).first()

    # Check if account is locked
    if user and user.is_locked:
        remaining = int((user.locked_until - datetime.utcnow()).total_seconds() / 60)
        return jsonify({
            'error': f'Account is locked. Try again in {remaining} minute(s).'
        }), 429

    if not user or not user.check_password(password):
        # Record failed login attempt
        if user:
            user.record_failed_login()
            AuditService.log_login(user.id, success=False, details={'reason': 'invalid_password'})
            db.session.commit()
        return jsonify({'error': 'Invalid username/email or password'}), 401

    if not user.is_active:
        AuditService.log_login(user.id, success=False, details={'reason': 'account_deactivated'})
        db.session.commit()
        return jsonify({'error': 'Account is deactivated'}), 403

    # Check if 2FA is enabled
    if user.totp_enabled:
        # Create a short-lived temporary token for 2FA verification
        # This token can only be used to complete 2FA, not to access resources
        temp_token = create_access_token(
            identity=user.id,
            additional_claims={'2fa_pending': True},
            expires_delta=False  # Use default (short) expiry
        )

        return jsonify({
            'requires_2fa': True,
            'temp_token': temp_token,
            'message': 'Two-factor authentication required'
        }), 200

    # No 2FA - proceed with normal login
    user.reset_failed_login()
    user.last_login_at = datetime.utcnow()
    db.session.commit()

    # Log successful login
    AuditService.log_login(user.id, success=True)
    db.session.commit()

    access_token = create_access_token(identity=user.id)
    refresh_token = create_refresh_token(identity=user.id)

    return jsonify({
        'user': user.to_dict(),
        'access_token': access_token,
        'refresh_token': refresh_token
    }), 200


@auth_bp.route('/refresh', methods=['POST'])
@jwt_required(refresh=True)
def refresh():
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)

    if not user or not user.is_active:
        return jsonify({'error': 'Invalid user'}), 401

    access_token = create_access_token(identity=current_user_id)

    return jsonify({
        'access_token': access_token
    }), 200


@auth_bp.route('/me', methods=['GET'])
@jwt_required()
def get_current_user():
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)

    if not user:
        return jsonify({'error': 'User not found'}), 404

    return jsonify({'user': user.to_dict()}), 200


@auth_bp.route('/me', methods=['PUT'])
@limiter.limit("3 per minute")
@jwt_required()
def update_current_user():
    current_user_id = get_jwt_identity()
    user = User.query.get(current_user_id)

    if not user:
        return jsonify({'error': 'User not found'}), 404

    data = request.get_json()

    if 'username' in data:
        existing = User.query.filter_by(username=data['username']).first()
        if existing and existing.id != user.id:
            return jsonify({'error': 'Username already taken'}), 409
        user.username = data['username']

    if 'email' in data:
        existing = User.query.filter(
            func.lower(User.email) == func.lower(data['email'])
        ).first()
        if existing and existing.id != user.id:
            return jsonify({'error': 'Email already registered'}), 409
        user.email = data['email']

    if 'password' in data:
        if len(data['password']) < 8:
            return jsonify({'error': 'Password must be at least 8 characters'}), 400
        user.set_password(data['password'])

    if 'sidebar_config' in data:
        config = data['sidebar_config']
        if isinstance(config, dict):
            preset = config.get('preset', 'full')
            valid_presets = ['full', 'web', 'email', 'devops', 'minimal', 'custom']
            if preset not in valid_presets:
                return jsonify({'error': f'Invalid sidebar preset: {preset}'}), 400
            hidden = config.get('hiddenItems', [])
            if not isinstance(hidden, list):
                return jsonify({'error': 'hiddenItems must be a list'}), 400
            user.set_sidebar_config({'preset': preset, 'hiddenItems': hidden})

    db.session.commit()

    return jsonify({'user': user.to_dict()}), 200


# ==========================================
# PASSKEY / WEBAUTHN
# ==========================================
@auth_bp.route('/passkeys/options/register', methods=['POST'])
@jwt_required()
def passkey_register_options():
    """Begin WebAuthn registration for the current user."""
    from app.services.passkey_service import PasskeyService
    user_id = get_jwt_identity()
    user = User.query.get(user_id)
    if not user:
        return jsonify({'error': 'User not found'}), 404
    options = PasskeyService.begin_registration(user)
    return jsonify(options), 200


@auth_bp.route('/passkeys/register', methods=['POST'])
@jwt_required()
def passkey_register():
    """Verify and save a new WebAuthn credential."""
    from app.services.passkey_service import PasskeyService
    user_id = get_jwt_identity()
    user = User.query.get(user_id)
    if not user:
        return jsonify({'error': 'User not found'}), 404
    data = request.get_json() or {}
    credential = data.get('credential')
    device_name = data.get('device_name', 'Passkey')
    if not credential:
        return jsonify({'error': 'No credential provided'}), 400
    result = PasskeyService.verify_registration(user, credential, device_name)
    return jsonify(result), 200 if result['success'] else 400


@auth_bp.route('/passkeys', methods=['GET'])
@jwt_required()
def passkey_list():
    """List the current user's passkeys."""
    from app.services.passkey_service import PasskeyService
    user_id = get_jwt_identity()
    return jsonify({'passkeys': PasskeyService.get_user_passkeys(user_id)}), 200


@auth_bp.route('/passkeys/<int:passkey_id>', methods=['DELETE'])
@jwt_required()
def passkey_delete(passkey_id):
    """Remove a passkey."""
    from app.services.passkey_service import PasskeyService
    user_id = get_jwt_identity()
    result = PasskeyService.remove_passkey(user_id, passkey_id)
    return jsonify(result), 200 if result['success'] else 404


@auth_bp.route('/passkeys/options/authenticate', methods=['POST'])
def passkey_auth_options():
    """Begin WebAuthn authentication (public, challenge only)."""
    from app.services.passkey_service import PasskeyService
    data = request.get_json() or {}
    user_id = data.get('user_id')
    user = User.query.get(user_id) if user_id else None
    options = PasskeyService.begin_authentication(user)
    return jsonify(options), 200


@auth_bp.route('/passkeys/authenticate', methods=['POST'])
@limiter.limit("5 per minute")
def passkey_authenticate():
    """Verify a WebAuthn assertion and issue JWT tokens."""
    from app.services.passkey_service import PasskeyService
    from app.services import sso_service
    data = request.get_json() or {}
    credential = data.get('credential')
    user_id = data.get('user_id')
    if not credential:
        return jsonify({'error': 'No credential provided'}), 400

    user = User.query.get(user_id) if user_id else None
    result = PasskeyService.verify_authentication(credential, user)
    if not result['success']:
        return jsonify(result), 401

    user = result['user']
    if not user.is_active:
        return jsonify({'error': 'Account is deactivated'}), 403

    user.reset_failed_login()
    user.last_login_at = datetime.utcnow()
    db.session.commit()

    AuditService.log_login(user.id, success=True, details={'method': 'passkey'})
    db.session.commit()

    access_token = create_access_token(identity=user.id)
    refresh_token = create_refresh_token(identity=user.id)

    return jsonify({
        'user': user.to_dict(),
        'access_token': access_token,
        'refresh_token': refresh_token
    }), 200
