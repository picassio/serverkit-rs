import { useState } from 'react';
import api from '../../services/api';
import Modal from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

// Create / credentials modals for the explorer. Logic is unchanged from the
// original Databases page; the explorer wires them to its toolbar and tree.
// Each modal component is mounted only while open, so Modal `open` is constant.
// The submit button stays inside <form>, so actions live in the body (not the
// Modal footer slot).

function CredentialsResult({ title, rows, onDone }) {
    return (
        <Modal open onClose={onDone} title={title}>
            <div className="credentials-box">
                <p>Save these credentials — the password won&apos;t be shown again.</p>
                {rows.map(([label, value]) => (
                    <div className="credential-item" key={label}>
                        <label>{label}:</label>
                        <code>{value}</code>
                    </div>
                ))}
            </div>
            <div className="modal-actions">
                <Button onClick={onDone}>Done</Button>
            </div>
        </Modal>
    );
}

export function CreateMySQLDatabaseModal({ onClose, onCreated }) {
    const [formData, setFormData] = useState({ name: '', charset: 'utf8mb4', collation: 'utf8mb4_unicode_ci', create_user: true, user_password: '' });
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');
    const [createdInfo, setCreatedInfo] = useState(null);

    async function handleSubmit(e) {
        e.preventDefault();
        setError('');
        setLoading(true);
        try {
            const result = await api.createMySQLDatabase(formData);
            if (result.success) {
                if (result.password) setCreatedInfo({ database: formData.name, user: result.user, password: result.password });
                else { onCreated(); onClose(); }
            }
        } catch (err) {
            setError(err.message || 'Failed to create database');
        } finally {
            setLoading(false);
        }
    }

    if (createdInfo) {
        return (
            <CredentialsResult
                title="Database created"
                rows={[['Database', createdInfo.database], ['Username', createdInfo.user], ['Password', createdInfo.password]]}
                onDone={() => { onCreated(); onClose(); }}
            />
        );
    }

    return (
        <Modal open onClose={onClose} title="Create MySQL database">
            {error && <div className="error-message">{error}</div>}
            <form onSubmit={handleSubmit}>
                <div className="form-group">
                    <label>Database name *</label>
                    <Input type="text" value={formData.name} onChange={(e) => setFormData({ ...formData, name: e.target.value })} placeholder="my_database" required pattern="[a-zA-Z0-9_]+" autoFocus />
                </div>
                <div className="form-row">
                    <div className="form-group">
                        <label>Character set</label>
                        <select value={formData.charset} onChange={(e) => setFormData({ ...formData, charset: e.target.value })}>
                            <option value="utf8mb4">utf8mb4</option>
                            <option value="utf8">utf8</option>
                            <option value="latin1">latin1</option>
                        </select>
                    </div>
                    <div className="form-group">
                        <label>Collation</label>
                        <select value={formData.collation} onChange={(e) => setFormData({ ...formData, collation: e.target.value })}>
                            <option value="utf8mb4_unicode_ci">utf8mb4_unicode_ci</option>
                            <option value="utf8mb4_general_ci">utf8mb4_general_ci</option>
                            <option value="utf8_general_ci">utf8_general_ci</option>
                        </select>
                    </div>
                </div>
                <div className="form-group">
                    <label className="checkbox-label">
                        <input type="checkbox" checked={formData.create_user} onChange={(e) => setFormData({ ...formData, create_user: e.target.checked })} />
                        Create user with same name and full privileges
                    </label>
                </div>
                <div className="modal-actions">
                    <Button type="button" variant="outline" onClick={onClose}>Cancel</Button>
                    <Button type="submit" disabled={loading}>{loading ? 'Creating…' : 'Create database'}</Button>
                </div>
            </form>
        </Modal>
    );
}

export function CreateMySQLUserModal({ databases, onClose, onCreated }) {
    const [formData, setFormData] = useState({ username: '', password: '', host: 'localhost', database: '', privileges: 'ALL' });
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');
    const [createdInfo, setCreatedInfo] = useState(null);

    async function generatePassword() {
        try {
            const result = await api.generateDatabasePassword();
            setFormData({ ...formData, password: result.password });
        } catch (err) {
            console.error('Failed to generate password:', err);
        }
    }

    async function handleSubmit(e) {
        e.preventDefault();
        setError('');
        setLoading(true);
        try {
            const result = await api.createMySQLUser(formData);
            if (result.success) setCreatedInfo({ username: formData.username, password: result.password, host: formData.host });
        } catch (err) {
            setError(err.message || 'Failed to create user');
        } finally {
            setLoading(false);
        }
    }

    if (createdInfo) {
        return (
            <CredentialsResult
                title="User created"
                rows={[['Username', createdInfo.username], ['Password', createdInfo.password], ['Host', createdInfo.host]]}
                onDone={() => { onCreated(); onClose(); }}
            />
        );
    }

    return (
        <Modal open onClose={onClose} title="Create MySQL user">
            {error && <div className="error-message">{error}</div>}
            <form onSubmit={handleSubmit}>
                <div className="form-group">
                    <label>Username *</label>
                    <Input type="text" value={formData.username} onChange={(e) => setFormData({ ...formData, username: e.target.value })} placeholder="db_user" required autoFocus />
                </div>
                <div className="form-group">
                    <label>Password</label>
                    <div className="input-with-button">
                        <Input type="text" value={formData.password} onChange={(e) => setFormData({ ...formData, password: e.target.value })} placeholder="Leave empty to auto-generate" />
                        <Button type="button" variant="outline" size="sm" onClick={generatePassword}>Generate</Button>
                    </div>
                </div>
                <div className="form-group">
                    <label>Host</label>
                    <select value={formData.host} onChange={(e) => setFormData({ ...formData, host: e.target.value })}>
                        <option value="localhost">localhost</option>
                        <option value="%">% (any host)</option>
                        <option value="127.0.0.1">127.0.0.1</option>
                    </select>
                </div>
                <div className="form-group">
                    <label>Grant privileges on database</label>
                    <select value={formData.database} onChange={(e) => setFormData({ ...formData, database: e.target.value })}>
                        <option value="">— None —</option>
                        {databases.map((db) => <option key={db.name} value={db.name}>{db.name}</option>)}
                    </select>
                </div>
                <div className="modal-actions">
                    <Button type="button" variant="outline" onClick={onClose}>Cancel</Button>
                    <Button type="submit" disabled={loading}>{loading ? 'Creating…' : 'Create user'}</Button>
                </div>
            </form>
        </Modal>
    );
}

export function CreatePostgreSQLDatabaseModal({ onClose, onCreated }) {
    const [formData, setFormData] = useState({ name: '', encoding: 'UTF8', create_user: true });
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');
    const [createdInfo, setCreatedInfo] = useState(null);

    async function handleSubmit(e) {
        e.preventDefault();
        setError('');
        setLoading(true);
        try {
            const result = await api.createPostgreSQLDatabase(formData);
            if (result.success) {
                if (result.password) setCreatedInfo({ database: formData.name, user: result.user, password: result.password });
                else { onCreated(); onClose(); }
            }
        } catch (err) {
            setError(err.message || 'Failed to create database');
        } finally {
            setLoading(false);
        }
    }

    if (createdInfo) {
        return (
            <CredentialsResult
                title="Database created"
                rows={[['Database', createdInfo.database], ['Username', createdInfo.user], ['Password', createdInfo.password]]}
                onDone={() => { onCreated(); onClose(); }}
            />
        );
    }

    return (
        <Modal open onClose={onClose} title="Create PostgreSQL database">
            {error && <div className="error-message">{error}</div>}
            <form onSubmit={handleSubmit}>
                <div className="form-group">
                    <label>Database name *</label>
                    <Input type="text" value={formData.name} onChange={(e) => setFormData({ ...formData, name: e.target.value })} placeholder="my_database" required autoFocus />
                </div>
                <div className="form-group">
                    <label>Encoding</label>
                    <select value={formData.encoding} onChange={(e) => setFormData({ ...formData, encoding: e.target.value })}>
                        <option value="UTF8">UTF8</option>
                        <option value="LATIN1">LATIN1</option>
                        <option value="SQL_ASCII">SQL_ASCII</option>
                    </select>
                </div>
                <div className="form-group">
                    <label className="checkbox-label">
                        <input type="checkbox" checked={formData.create_user} onChange={(e) => setFormData({ ...formData, create_user: e.target.checked })} />
                        Create user with same name and full privileges
                    </label>
                </div>
                <div className="modal-actions">
                    <Button type="button" variant="outline" onClick={onClose}>Cancel</Button>
                    <Button type="submit" disabled={loading}>{loading ? 'Creating…' : 'Create database'}</Button>
                </div>
            </form>
        </Modal>
    );
}

export function CreatePostgreSQLUserModal({ databases, onClose, onCreated }) {
    const [formData, setFormData] = useState({ username: '', password: '', database: '' });
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');
    const [createdInfo, setCreatedInfo] = useState(null);

    async function generatePassword() {
        try {
            const result = await api.generateDatabasePassword();
            setFormData({ ...formData, password: result.password });
        } catch (err) {
            console.error('Failed to generate password:', err);
        }
    }

    async function handleSubmit(e) {
        e.preventDefault();
        setError('');
        setLoading(true);
        try {
            const result = await api.createPostgreSQLUser(formData);
            if (result.success) setCreatedInfo({ username: formData.username, password: result.password });
        } catch (err) {
            setError(err.message || 'Failed to create user');
        } finally {
            setLoading(false);
        }
    }

    if (createdInfo) {
        return (
            <CredentialsResult
                title="User created"
                rows={[['Username', createdInfo.username], ['Password', createdInfo.password]]}
                onDone={() => { onCreated(); onClose(); }}
            />
        );
    }

    return (
        <Modal open onClose={onClose} title="Create PostgreSQL user">
            {error && <div className="error-message">{error}</div>}
            <form onSubmit={handleSubmit}>
                <div className="form-group">
                    <label>Username *</label>
                    <Input type="text" value={formData.username} onChange={(e) => setFormData({ ...formData, username: e.target.value })} placeholder="db_user" required autoFocus />
                </div>
                <div className="form-group">
                    <label>Password</label>
                    <div className="input-with-button">
                        <Input type="text" value={formData.password} onChange={(e) => setFormData({ ...formData, password: e.target.value })} placeholder="Leave empty to auto-generate" />
                        <Button type="button" variant="outline" size="sm" onClick={generatePassword}>Generate</Button>
                    </div>
                </div>
                <div className="form-group">
                    <label>Grant privileges on database</label>
                    <select value={formData.database} onChange={(e) => setFormData({ ...formData, database: e.target.value })}>
                        <option value="">— None —</option>
                        {databases.map((db) => <option key={db.name} value={db.name}>{db.name}</option>)}
                    </select>
                </div>
                <div className="modal-actions">
                    <Button type="button" variant="outline" onClick={onClose}>Cancel</Button>
                    <Button type="submit" disabled={loading}>{loading ? 'Creating…' : 'Create user'}</Button>
                </div>
            </form>
        </Modal>
    );
}
