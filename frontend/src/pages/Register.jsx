import { useState, useEffect } from 'react';
import { useNavigate, Link, useSearchParams } from 'react-router-dom';
import { useAuth } from '../contexts/AuthContext';
import api from '../services/api';
import ServerKitLogo from '../components/ServerKitLogo';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

const Register = () => {
    const [email, setEmail] = useState('');
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(false);
    const [inviteInfo, setInviteInfo] = useState(null);
    const [inviteLoading, setInviteLoading] = useState(false);
    const [inviteInvalid, setInviteInvalid] = useState(false);
    const { register, registrationEnabled } = useAuth();
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const inviteToken = searchParams.get('invite');

    useEffect(() => {
        if (inviteToken) {
            setInviteLoading(true);
            api.validateInvitation(inviteToken)
                .then(data => {
                    setInviteInfo(data);
                    if (data.email) setEmail(data.email);
                })
                .catch(() => {
                    setInviteInvalid(true);
                })
                .finally(() => setInviteLoading(false));
        }
    }, [inviteToken]);

    // If no invite token and registration disabled, show message
    if (!inviteToken && !registrationEnabled) {
        return (
            <div className="auth-container">
                <div className="auth-card">
                    <div className="auth-header">
                        <div className="brand-logo">
                            <ServerKitLogo width={40} height={40} />
                        </div>
                        <h1>ServerKit</h1>
                        <p>Registration is currently disabled</p>
                    </div>
                    <p className="auth-footer">
                        Already have an account? <Link to="/login">Sign in</Link>
                    </p>
                </div>
            </div>
        );
    }

    if (inviteLoading) {
        return (
            <div className="auth-container">
                <div className="auth-card">
                    <div className="auth-header">
                        <div className="brand-logo">
                            <ServerKitLogo width={40} height={40} />
                        </div>
                        <h1>ServerKit</h1>
                        <p>Validating invitation...</p>
                    </div>
                </div>
            </div>
        );
    }

    if (inviteInvalid) {
        return (
            <div className="auth-container">
                <div className="auth-card">
                    <div className="auth-header">
                        <div className="brand-logo">
                            <ServerKitLogo width={40} height={40} />
                        </div>
                        <h1>ServerKit</h1>
                        <p>This invitation is invalid or has expired</p>
                    </div>
                    <p className="auth-footer">
                        Already have an account? <Link to="/login">Sign in</Link>
                    </p>
                </div>
            </div>
        );
    }

    async function handleSubmit(e) {
        e.preventDefault();
        setError('');

        if (password !== confirmPassword) {
            setError('Passwords do not match');
            return;
        }

        if (password.length < 8) {
            setError('Password must be at least 8 characters');
            return;
        }

        setLoading(true);

        try {
            await register(email, username, password, inviteToken || undefined);
            navigate('/');
        } catch (err) {
            setError(err.message || 'Failed to register');
        } finally {
            setLoading(false);
        }
    }

    return (
        <div className="auth-container">
            <div className="auth-card">
                <div className="auth-header">
                    <div className="brand-logo">
                        <ServerKitLogo width={40} height={40} />
                    </div>
                    <h1>ServerKit</h1>
                    <p>{inviteInfo ? `You've been invited as ${inviteInfo.role}` : 'Create your account'}</p>
                </div>

                {error && <div className="error-message">{error}</div>}

                <form onSubmit={handleSubmit}>
                    <div className="form-group">
                        <Label htmlFor="email">Email</Label>
                        <Input
                            type="email"
                            id="email"
                            value={email}
                            onChange={(e) => setEmail(e.target.value)}
                            placeholder="you@example.com"
                            required
                            readOnly={!!inviteInfo?.email}
                        />
                    </div>

                    <div className="form-group">
                        <Label htmlFor="username">Username</Label>
                        <Input
                            type="text"
                            id="username"
                            value={username}
                            onChange={(e) => setUsername(e.target.value)}
                            placeholder="Choose a username"
                            required
                        />
                    </div>

                    <div className="form-group">
                        <Label htmlFor="password">Password</Label>
                        <Input
                            type="password"
                            id="password"
                            value={password}
                            onChange={(e) => setPassword(e.target.value)}
                            placeholder="At least 8 characters"
                            required
                        />
                    </div>

                    <div className="form-group">
                        <Label htmlFor="confirmPassword">Confirm Password</Label>
                        <Input
                            type="password"
                            id="confirmPassword"
                            value={confirmPassword}
                            onChange={(e) => setConfirmPassword(e.target.value)}
                            placeholder="Confirm your password"
                            required
                        />
                    </div>

                    <Button type="submit" className="btn-full" disabled={loading}>
                        {loading ? 'Creating account...' : 'Create Account'}
                    </Button>
                </form>

                <p className="auth-footer">
                    Already have an account? <Link to="/login">Sign in</Link>
                </p>
            </div>
        </div>
    );
};

export default Register;
