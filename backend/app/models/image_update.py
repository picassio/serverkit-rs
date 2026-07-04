from datetime import datetime
from app import db


class ImageUpdateCheck(db.Model):
    """Result of comparing an application's running image digest against the
    current digest for the same tag in its registry."""
    __tablename__ = 'image_update_checks'

    id = db.Column(db.Integer, primary_key=True)
    application_id = db.Column(db.Integer, db.ForeignKey('applications.id'), nullable=False, index=True)
    image_ref = db.Column(db.String(500), nullable=False)
    current_digest = db.Column(db.String(128))    # sha256:... running locally
    latest_digest = db.Column(db.String(128))     # sha256:... currently in the registry
    update_available = db.Column(db.Boolean, default=False, nullable=False)
    status = db.Column(db.String(20), default='pending')  # pending, completed, failed
    error_message = db.Column(db.Text)
    checked_at = db.Column(db.DateTime, default=datetime.utcnow)

    application = db.relationship('Application', backref=db.backref(
        'image_update_checks', lazy='dynamic',
        order_by='ImageUpdateCheck.checked_at.desc()'))

    def to_dict(self):
        return {
            'id': self.id,
            'application_id': self.application_id,
            'image_ref': self.image_ref,
            'current_digest': self.current_digest,
            'latest_digest': self.latest_digest,
            'update_available': self.update_available,
            'status': self.status,
            'error_message': self.error_message,
            'checked_at': self.checked_at.isoformat() if self.checked_at else None,
        }
