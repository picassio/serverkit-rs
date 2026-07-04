# Backup Protection

ServerKit runs on your server, so backups can be **scheduled, smart, and
one-click restorable** — no plugins, no PHP timeouts, no middlemen. The
**Protection** panel turns the old "save a snapshot" button into real
protection for both WordPress sites and applications (Services).

You'll find it in two equivalent places (both render the same panel):

- **WordPress**: site detail → top **Backups** tab, or **Settings → Backups**.
- **Services**: application detail → **Settings → Backups**.

The global **Backups** page remains the archive / storage-management and
cost-settings view.

## What "protection" means

- **Automatic** — one toggle turns on a daily (or custom) schedule.
- **Visible** — a list *and* a calendar show what succeeded and what failed.
- **Measurable** — every backup shows its size and an estimated storage cost.
- **Restorable** — one-click restore (full / files only / database only), with a
  safety backup taken first by default.
- **Smart** — periodic full backups plus daily incrementals, retention, and
  compression tiers.
- **Connected** — each run is a job on the unified job bus (visible in **Jobs**),
  and failures raise a notification.

## The three cards

1. **Protection status** — the master on/off switch, a status pill
   (Protected / Pending / Failed / Off), KPIs (next backup, monthly cost,
   storage used), and a **Back up now** button (works even when protection is
   off — it takes a one-off backup).
2. **Schedule** — frequency (Daily / Weekly / Custom with a cron preview),
   retention (keep last *N* backups **and** delete older than *N* days — both
   rules apply), how often a **full** backup is taken, the compression tier, and
   an optional **remote copy**.
3. **Backup history** — a list or calendar of every run. Click a row for a
   detail drawer (metadata, cost, verify/restore/delete); restore opens a
   drawer where you pick the scope and safety options.

## Smart backups

- **Full vs incremental** (applications): the first run, and every
  `full_every_n_days`, is a **full** backup; the days in between are
  **incremental** (`tar --listed-incremental`), so daily storage growth stays
  small. Restoring an incremental automatically replays its full + intervening
  increments.
- **Compression tiers**: `fast` (gzip), `balanced` (zstd ‑3, the default), and
  `max` (zstd ‑19). If `zstd` isn't installed, ServerKit falls back to gzip.
- **Retention**: a backup is kept only if it's within the last *N* backups **and**
  newer than *N* days. The most recent successful backup is **never** deleted,
  and the full/increments a kept backup depends on are protected from cleanup.
- **Remote copy + verify**: when enabled, each run is uploaded to your configured
  remote (S3 / B2) and the primary archive is verified (size + checksum).

## Cost

ServerKit is free and open-source — **"cost" means your own storage cost, not a
ServerKit charge**:

- **Local** = your server's disk. It's **free by default** ($0/GB); set a
  `$/GB/month` rate only if you want to attribute server-disk cost.
- **S3 / B2** = your real cloud-provider bill (sensible list-price defaults:
  S3 `$0.023`, B2 `$0.006` per GB/month).

Set the rates under **Backups → Settings → Storage cost rates**. The panel shows
both the raw **storage used** (GB on your server) and an estimated **monthly
cost**; the global page adds an **estimated** and **projected** (at full
retention) monthly cost.

## Restore (≤ 3 clicks)

Open a backup from the history → **Restore** → choose the scope and confirm:

- **Full site** (files + database), **Files only**, or **Database only**
  (WordPress). Applications restore files.
- **Create a safety backup first** (on by default) snapshots the current state
  before overwriting.
- **Maintenance mode** during restore (WordPress, on by default).

Restore runs as a `restore.run` job; you're notified on completion.

## Under the hood

- **Models**: `BackupPolicy` (one per target) and `BackupRun` (one per run, the
  history's source of truth) — `backend/app/models/backup_policy.py`,
  `backup_run.py`.
- **Services**: `backup_policy_service.py` (policy CRUD, schedule wiring, the
  `backup.policy.run` / `restore.run` job handlers, retention),
  `backup_cost_service.py` (cost), and the smart-backup helpers in
  `backup_service.py`.
- **Scheduling**: the policy's cron is mirrored into a `ScheduledJob` on the
  unified job bus, so firing is observable and retryable like any other job.
