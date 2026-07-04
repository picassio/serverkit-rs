import { useState } from 'react';
import { useAuth } from '../../contexts/AuthContext';
import { Info } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

const SetupStepAccount = ({ onComplete }) => {
    const [email, setEmail] = useState('');
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(false);
    const { register, login, registrationEnabled } = useAuth();

    // If users already exist (e.g. admin created via CLI), show login form instead
    const showLogin = !registrationEnabled;

    async function handleRegister(e) {
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
            await register(email, username, password);
            onComplete({ email, username });
        } catch (err) {
            setError(err.message || 'Failed to create admin account');
        } finally {
            setLoading(false);
        }
    }

    async function handleLogin(e) {
        e.preventDefault();
        setError('');
        setLoading(true);

        try {
            await login(email, password);
            onComplete({ email, username: email });
        } catch (err) {
            setError(err.message || 'Failed to sign in');
        } finally {
            setLoading(false);
        }
    }

    if (showLogin) {
        return (
            <div className="wizard-step">
                <h2 className="wizard-step-title">Sign In</h2>
                <p className="wizard-step-description">
                    An admin account already exists. Sign in to continue setup.
                </p>

                <div className="alert alert-info">
                    <Info size={20} />
                    <p>
                        It looks like an admin account was created via the CLI.
                        Sign in with those credentials to finish setting up your server.
                    </p>
                </div>

                {error && <div className="error-message">{error}</div>}

                <form onSubmit={handleLogin}>
                    <div className="form-group">
                        <Label htmlFor="email">Email</Label>
                        <Input
                            type="email"
                            id="email"
                            value={email}
                            onChange={(e) => setEmail(e.target.value)}
                            placeholder="admin@example.com"
                            required
                            autoFocus
                        />
                    </div>

                    <div className="form-group">
                        <Label htmlFor="password">Password</Label>
                        <Input
                            type="password"
                            id="password"
                            value={password}
                            onChange={(e) => setPassword(e.target.value)}
                            placeholder="Enter your password"
                            required
                        />
                    </div>

                    <button
                        type="submit"
                        className="btn-wizard-next"
                        disabled={loading}
                        style={{ width: '100%', justifyContent: 'center' }}
                    >
                        {loading ? 'Signing in...' : 'Sign In & Continue'}
                    </button>
                </form>
            </div>
        );
    }

    return (
        <div className="wizard-step">
            <h2 className="wizard-step-title">Create Admin Account</h2>
            <p className="wizard-step-description">
                Set up the administrator account for your server.
            </p>

            <div className="alert alert-info">
                <Info size={20} />
                <p>
                    This is your first time using ServerKit. Create an administrator
                    account to get started. This account will have full access to
                    manage your server.
                </p>
            </div>

            {error && <div className="error-message">{error}</div>}

            <form onSubmit={handleRegister}>
                <div className="form-group">
                    <Label htmlFor="email">Admin Email</Label>
                    <Input
                        type="email"
                        id="email"
                        value={email}
                        onChange={(e) => setEmail(e.target.value)}
                        placeholder="admin@example.com"
                        required
                        autoFocus
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

                <button
                    type="submit"
                    className="btn-wizard-next"
                    disabled={loading}
                    style={{ width: '100%', justifyContent: 'center' }}
                >
                    {loading ? 'Creating account...' : 'Continue'}
                </button>
            </form>
        </div>
    );
};

export default SetupStepAccount;
