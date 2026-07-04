import { useState, useEffect } from 'react';
import { useAuth } from '../../contexts/AuthContext';
import api from '../../services/api';
import PermissionEditor from './PermissionEditor';
import Modal from '../Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Checkbox } from '@/components/ui/checkbox';
import { Select, SelectTrigger, SelectContent, SelectItem, SelectValue } from '@/components/ui/select';

const UserModal = ({ user, onSave, onClose }) => {
    const [formData, setFormData] = useState({
        email: '',
        username: '',
        password: '',
        confirmPassword: '',
        role: 'developer',
        is_active: true
    });
    const [permissions, setPermissions] = useState({});
    const [showPermissions, setShowPermissions] = useState(false);
    const [templates, setTemplates] = useState({});
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(false);
    const { user: currentUser } = useAuth();

    const isEditing = !!user;
    const isSelf = user?.id === currentUser?.id;

    useEffect(() => {
        if (user) {
            setFormData({
                email: user.email || '',
                username: user.username || '',
                password: '',
                confirmPassword: '',
                role: user.role || 'developer',
                is_active: user.is_active !== false
            });
            if (user.permissions) {
                setPermissions(user.permissions);
                // Show permissions section if user has custom permissions set
                const hasCustom = user.permissions && Object.keys(user.permissions).length > 0;
                setShowPermissions(hasCustom);
            }
        }
    }, [user]);

    useEffect(() => {
        api.getPermissionTemplates().then(data => {
            setTemplates(data.templates || {});
        }).catch(() => {});
    }, []);

    function handleChange(e) {
        const { name, value, type, checked } = e.target;
        setFormData(prev => ({
            ...prev,
            [name]: type === 'checkbox' ? checked : value
        }));
        // When role changes, load template defaults for the permissions editor
        if (name === 'role' && templates[value]) {
            setPermissions(templates[value]);
        }
    }

    function handleRoleChange(newRole) {
        setFormData(prev => ({ ...prev, role: newRole }));
        if (templates[newRole]) {
            setPermissions(templates[newRole]);
        }
    }

    async function handleSubmit(e) {
        e.preventDefault();
        setError('');

        // Validation
        if (!formData.email || !formData.username) {
            setError('Email and username are required');
            return;
        }

        if (!isEditing && !formData.password) {
            setError('Password is required for new users');
            return;
        }

        if (formData.password && formData.password.length < 8) {
            setError('Password must be at least 8 characters');
            return;
        }

        if (formData.password && formData.password !== formData.confirmPassword) {
            setError('Passwords do not match');
            return;
        }

        setLoading(true);

        try {
            const userData = {
                email: formData.email,
                username: formData.username,
                role: formData.role,
                is_active: formData.is_active
            };

            // Only include password if it's been set
            if (formData.password) {
                userData.password = formData.password;
            }

            // Include custom permissions if editor is open and role isn't admin
            if (showPermissions && formData.role !== 'admin') {
                userData.permissions = permissions;
            }

            await onSave(userData);
        } catch (err) {
            setError(err.message || 'Failed to save user');
        } finally {
            setLoading(false);
        }
    }

    return (
        <Modal open={true} onClose={onClose} title={isEditing ? 'Edit User' : 'Add New User'} size="md">
                <form onSubmit={handleSubmit}>
                    <div className="modal-body">
                        {error && <div className="error-message">{error}</div>}

                        <div className="form-group">
                            <Label htmlFor="email">Email</Label>
                            <Input
                                type="email"
                                id="email"
                                name="email"
                                value={formData.email}
                                onChange={handleChange}
                                placeholder="user@example.com"
                                required
                            />
                        </div>

                        <div className="form-group">
                            <Label htmlFor="username">Username</Label>
                            <Input
                                type="text"
                                id="username"
                                name="username"
                                value={formData.username}
                                onChange={handleChange}
                                placeholder="Enter username"
                                required
                            />
                        </div>

                        <div className="form-row">
                            <div className="form-group">
                                <Label htmlFor="password">
                                    {isEditing ? 'New Password (leave blank to keep current)' : 'Password'}
                                </Label>
                                <Input
                                    type="password"
                                    id="password"
                                    name="password"
                                    value={formData.password}
                                    onChange={handleChange}
                                    placeholder={isEditing ? 'Leave blank to keep current' : 'At least 8 characters'}
                                    required={!isEditing}
                                />
                            </div>

                            <div className="form-group">
                                <Label htmlFor="confirmPassword">Confirm Password</Label>
                                <Input
                                    type="password"
                                    id="confirmPassword"
                                    name="confirmPassword"
                                    value={formData.confirmPassword}
                                    onChange={handleChange}
                                    placeholder="Confirm password"
                                    required={!!formData.password}
                                />
                            </div>
                        </div>

                        <div className="form-group">
                            <Label htmlFor="role">Role</Label>
                            <Select
                                value={formData.role}
                                onValueChange={handleRoleChange}
                                disabled={isSelf}
                            >
                                <SelectTrigger id="role">
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="admin">Admin - Full access</SelectItem>
                                    <SelectItem value="developer">Developer - Manage apps and deployments</SelectItem>
                                    <SelectItem value="viewer">Viewer - Read-only access</SelectItem>
                                </SelectContent>
                            </Select>
                            {isSelf && (
                                <span className="form-help">You cannot change your own role</span>
                            )}
                        </div>

                        <div className="form-group">
                            <label className="checkbox-label">
                                <Checkbox
                                    name="is_active"
                                    checked={formData.is_active}
                                    onCheckedChange={(checked) => setFormData(prev => ({ ...prev, is_active: checked }))}
                                    disabled={isSelf}
                                />
                                <span className="checkbox-text">Account is active</span>
                            </label>
                            {isSelf && (
                                <span className="form-help">You cannot deactivate your own account</span>
                            )}
                        </div>

                        <div className="role-descriptions">
                            <h4>Role Permissions</h4>
                            <div className="role-item">
                                <span className="role-name">Admin</span>
                                <span className="role-desc">Full system access including user management and settings</span>
                            </div>
                            <div className="role-item">
                                <span className="role-name">Developer</span>
                                <span className="role-desc">Manage applications, deployments, databases, and domains</span>
                            </div>
                            <div className="role-item">
                                <span className="role-name">Viewer</span>
                                <span className="role-desc">Read-only access to dashboards and logs</span>
                            </div>
                        </div>

                        {formData.role !== 'admin' && (
                            <div className="customize-permissions-section">
                                <Button
                                    type="button"
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => {
                                        if (!showPermissions && templates[formData.role]) {
                                            setPermissions(templates[formData.role]);
                                        }
                                        setShowPermissions(!showPermissions);
                                    }}
                                >
                                    {showPermissions ? 'Hide' : 'Customize'} Permissions
                                    <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" fill="none" strokeWidth="2" style={{ marginLeft: 4 }}>
                                        {showPermissions
                                            ? <polyline points="18 15 12 9 6 15"/>
                                            : <polyline points="6 9 12 15 18 9"/>
                                        }
                                    </svg>
                                </Button>
                                {showPermissions && (
                                    <PermissionEditor
                                        permissions={permissions}
                                        onChange={setPermissions}
                                    />
                                )}
                            </div>
                        )}
                    </div>

                    <div className="modal-footer">
                        <Button type="button" variant="ghost" onClick={onClose}>
                            Cancel
                        </Button>
                        <Button type="submit" variant="default" disabled={loading}>
                            {loading ? 'Saving...' : (isEditing ? 'Save Changes' : 'Create User')}
                        </Button>
                    </div>
                </form>
        </Modal>
    );
};

export default UserModal;
