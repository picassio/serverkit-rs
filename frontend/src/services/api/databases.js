// Database CRUD, MySQL, PostgreSQL, SQLite, Docker databases, queries

// Database status
export async function getDatabaseStatus() {
    return this.request('/databases/status');
}

// MySQL
export async function getMySQLDatabases(rootPassword = null) {
    const params = rootPassword ? `?root_password=${encodeURIComponent(rootPassword)}` : '';
    return this.request(`/databases/mysql${params}`);
}

export async function createMySQLDatabase(data) {
    return this.request('/databases/mysql', {
        method: 'POST',
        body: data
    });
}

export async function dropMySQLDatabase(name, rootPassword = null) {
    return this.request(`/databases/mysql/${name}`, {
        method: 'DELETE',
        body: { root_password: rootPassword }
    });
}

export async function getMySQLTables(database, rootPassword = null) {
    const params = rootPassword ? `?root_password=${encodeURIComponent(rootPassword)}` : '';
    return this.request(`/databases/mysql/${database}/tables${params}`);
}

export async function backupMySQLDatabase(database, rootPassword = null) {
    return this.request(`/databases/mysql/${database}/backup`, {
        method: 'POST',
        body: { root_password: rootPassword }
    });
}

export async function restoreMySQLDatabase(database, backupPath, rootPassword = null) {
    return this.request(`/databases/mysql/${database}/restore`, {
        method: 'POST',
        body: { backup_path: backupPath, root_password: rootPassword }
    });
}

export async function getMySQLUsers(rootPassword = null) {
    const params = rootPassword ? `?root_password=${encodeURIComponent(rootPassword)}` : '';
    return this.request(`/databases/mysql/users${params}`);
}

export async function createMySQLUser(data) {
    return this.request('/databases/mysql/users', {
        method: 'POST',
        body: data
    });
}

export async function dropMySQLUser(username, host = 'localhost', rootPassword = null) {
    return this.request(`/databases/mysql/users/${username}`, {
        method: 'DELETE',
        body: { host, root_password: rootPassword }
    });
}

export async function grantMySQLPrivileges(username, database, privileges = 'ALL', host = 'localhost', rootPassword = null) {
    return this.request(`/databases/mysql/users/${username}/grant`, {
        method: 'POST',
        body: { database, privileges, host, root_password: rootPassword }
    });
}

// PostgreSQL
export async function getPostgreSQLDatabases() {
    return this.request('/databases/postgresql');
}

export async function createPostgreSQLDatabase(data) {
    return this.request('/databases/postgresql', {
        method: 'POST',
        body: data
    });
}

export async function dropPostgreSQLDatabase(name) {
    return this.request(`/databases/postgresql/${name}`, { method: 'DELETE' });
}

export async function getPostgreSQLTables(database) {
    return this.request(`/databases/postgresql/${database}/tables`);
}

export async function backupPostgreSQLDatabase(database) {
    return this.request(`/databases/postgresql/${database}/backup`, { method: 'POST' });
}

export async function restorePostgreSQLDatabase(database, backupPath) {
    return this.request(`/databases/postgresql/${database}/restore`, {
        method: 'POST',
        body: { backup_path: backupPath }
    });
}

export async function getPostgreSQLUsers() {
    return this.request('/databases/postgresql/users');
}

export async function createPostgreSQLUser(data) {
    return this.request('/databases/postgresql/users', {
        method: 'POST',
        body: data
    });
}

export async function dropPostgreSQLUser(username) {
    return this.request(`/databases/postgresql/users/${username}`, { method: 'DELETE' });
}

export async function grantPostgreSQLPrivileges(username, database, privileges = 'ALL') {
    return this.request(`/databases/postgresql/users/${username}/grant`, {
        method: 'POST',
        body: { database, privileges }
    });
}

// Backups
export async function getDatabaseBackups(type = null) {
    const params = type ? `?type=${type}` : '';
    return this.request(`/databases/backups${params}`);
}

export async function deleteDatabaseBackup(filename) {
    return this.request(`/databases/backups/${filename}`, { method: 'DELETE' });
}

export async function generateDatabasePassword(length = 16) {
    return this.request(`/databases/generate-password?length=${length}`);
}

// Query Execution
export async function executeMySQLQuery(database, query, readonly = true) {
    return this.request(`/databases/mysql/${database}/query`, {
        method: 'POST',
        body: { query, readonly }
    });
}

export async function executePostgreSQLQuery(database, query, readonly = true) {
    return this.request(`/databases/postgresql/${database}/query`, {
        method: 'POST',
        body: { query, readonly }
    });
}

export async function executeSQLiteQuery(path, query, readonly = true) {
    return this.request('/databases/sqlite/query', {
        method: 'POST',
        body: { path, query, readonly }
    });
}

export async function getMySQLTableStructure(database, table) {
    return this.request(`/databases/mysql/${database}/tables/${table}/structure`);
}

export async function getPostgreSQLTableStructure(database, table) {
    return this.request(`/databases/postgresql/${database}/tables/${table}/structure`);
}

export async function getSQLiteTableStructure(path, table) {
    return this.request(`/databases/sqlite/tables/${table}/structure?path=${encodeURIComponent(path)}`);
}

export async function getSQLiteDatabases() {
    return this.request('/databases/sqlite');
}

export async function getSQLiteTables(path) {
    return this.request(`/databases/sqlite/tables?path=${encodeURIComponent(path)}`);
}

// Docker Container Databases
export async function getDockerDatabases() {
    return this.request('/databases/docker');
}

// Flat list of databases discovered across all Docker apps, tagged with engine
// + owning app — used to surface containerised DBs under their engine node.
export async function getAllDockerDatabases() {
    return this.request('/databases/docker/databases');
}

export async function getAppDatabases(appId) {
    return this.request(`/databases/docker/app/${appId}`);
}

export async function getDockerContainerDatabases(container, password = null) {
    const headers = password ? { 'X-DB-Password': password } : {};
    return this.request(`/databases/docker/${container}/databases`, { headers });
}

export async function getDockerDatabaseTables(container, database, password = null, user = null) {
    const headers = password ? { 'X-DB-Password': password } : {};
    const params = user ? `?user=${encodeURIComponent(user)}` : '';
    return this.request(`/databases/docker/${container}/${database}/tables${params}`, { headers });
}

export async function executeDockerQuery(container, database, query, password = null, readonly = true, user = null) {
    const headers = password ? { 'X-DB-Password': password } : {};
    return this.request(`/databases/docker/${container}/${database}/query`, {
        method: 'POST',
        body: { query, readonly, password, user },
        headers
    });
}

// Database Migrations
export async function getMigrationStatus() {
    return this.request('/migrations/status');
}

export async function createMigrationBackup() {
    return this.request('/migrations/backup', { method: 'POST' });
}

export async function applyMigrations() {
    return this.request('/migrations/apply', { method: 'POST' });
}

export async function getMigrationHistory() {
    return this.request('/migrations/history');
}

// Managed databases — durable tracking beside the live introspection. Secrets
// are never returned (has_secret); the connection URI is revealed only via an
// explicit, audited action.
export async function getManagedDatabases() {
    return this.request('/databases/managed');
}

export async function getManagedDatabase(id) {
    return this.request(`/databases/managed/${id}`);
}

export async function createManagedDatabase(data) {
    return this.request('/databases/managed', { method: 'POST', body: data });
}

export async function adoptManagedDatabase(data) {
    return this.request('/databases/managed/adopt', { method: 'POST', body: data });
}

export async function deleteManagedDatabase(id, { drop = false } = {}) {
    const q = drop ? '?drop=true' : '';
    return this.request(`/databases/managed/${id}${q}`, { method: 'DELETE' });
}

export async function revealManagedConnectionUri(id) {
    return this.request(`/databases/managed/${id}/connection-uri`, { method: 'POST' });
}

export async function protectManagedDatabase(id, policy = null) {
    return this.request(`/databases/managed/${id}/protect`, { method: 'POST', body: { policy } });
}
