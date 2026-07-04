# backend-rs — ServerKit backend in Rust

Full 1:1 port of the Flask backend (`../backend`) to Rust. The React frontend
(`../frontend`) runs **unmodified** against this server: same REST contracts,
same JWT semantics, same Socket.IO wire protocol.

## Stack

- **axum** + **tokio** — HTTP
- **socketioxide** — Socket.IO server (wire-compatible with socket.io-client v4)
- **sqlx** (SQLite) — same database file/schema as the Flask backend
- **jsonwebtoken** — flask-jwt-extended-compatible HS256 tokens
- **scrypt/pbkdf2** — werkzeug-compatible password hashes

## Compatibility invariants (do not break)

1. **Passwords**: werkzeug format `method$salt$hex` (`scrypt:32768:8:1`, dklen=64).
   Hashes are portable in both directions between Python and Rust.
2. **JWTs**: HS256, claims `{fresh, iat, jti, type, sub, nbf, exp}`; `sub` is the
   user id **as a string** (PyJWT ≥2.10 requirement). Decode accepts int subs
   from older Flask tokens.
3. **Datetimes**: stored `YYYY-MM-DD HH:MM:SS.ffffff` (SQLAlchemy style),
   serialized `YYYY-MM-DDTHH:MM:SS.ffffff` (Python `isoformat()`).
4. **Errors**: `{"error": "message"}` + HTTP status, matching Flask handlers.
5. **Schema**: `migrations/0001_init.sql` is the Alembic baseline at revision
   `047_agent_footprint_dirs` (124 tables). Databases created by Flask are
   detected and left untouched (migrator skipped).

## Crates

| Crate | Ports |
|---|---|
| `sk-core` | config.py env contract, DB pool, SQLAlchemy time formats |
| `sk-auth` | werkzeug password hashing, flask-jwt-extended JWTs |
| `sk-models` | models/* + to_dict() JSON parity (P0: users, settings, permissions) |
| `sk-server` | app factory + api/* blueprints (P0: auth), Socket.IO gateway |

Planned (see wiki `serverkit-exploration-magento-fork`): sk-system, sk-nginx,
sk-php, sk-ssl, sk-docker, sk-db, sk-apps, sk-deploy, sk-backups, sk-cron,
sk-files, sk-monitoring, sk-terminal, sk-fleet, sk-templates, sk-workflows,
sk-plugin-host (WASM), sk-magento. AI assistant runs as a pi-SDK Node sidecar.

## Run

```bash
DATABASE_URL=sqlite:///path/to/serverkit.db PORT=5000 cargo run -p sk-server
```

Env vars mirror the Flask backend: `DATABASE_URL`, `SECRET_KEY`,
`JWT_SECRET_KEY`, `PORT`, plus `SK_FRONTEND_DIST` (default `../frontend/dist`).

## Oracle testing

The Flask backend is the behavioral oracle during the port. Verified so far:
werkzeug-hashed users log in through Rust; Rust tokens decode with PyJWT;
engine.io handshake accepted. As endpoint groups are ported, replay captured
frontend traffic against both backends and diff the JSON.

## P0 status

- [x] Schema baseline (124 tables) + fresh-DB migrator / existing-DB detection
- [x] `/api/v1/auth`: setup-status, register, login (incl. lockout + 2FA-pending
      shape), refresh, me (GET/PUT), complete-onboarding
- [x] Socket.IO connect with JWT auth
- [x] Static SPA serving
- [ ] TODO(P1): audit logs, rate limiting, TOTP verify endpoint, SSO, invitations
