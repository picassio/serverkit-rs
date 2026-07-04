---
name: deploy
description: Deploy ServerKit to the production server via SSH. Runs update, rebuilds frontend, restarts services, and performs health checks. Use when pushing changes to production.
disable-model-invocation: true
argument-hint: "[branch]"
allowed-tools: mcp__ssh-mcp__exec, mcp__ssh-mcp__sudo-exec, Bash
---

Deploy ServerKit to the production server. Target branch: **${ARGUMENTS:-main}**

## Pre-deploy Checks

1. Check the current git status locally — warn if there are uncommitted changes
2. Check if the local branch is ahead of remote — warn if unpushed commits exist

## Deploy Steps

Run these on the remote server via SSH MCP:

### 1. Check current state
```
serverkit version
serverkit status
```

### 2. Run the update
```
serverkit update --branch ${ARGUMENTS:-main}
```

This will:
- Stop services
- Pull latest code from the target branch
- Update Python dependencies
- Sync app templates
- Rebuild frontend Docker container
- Restart all services

### 3. Health check (wait 10 seconds after update)
```
curl -s http://127.0.0.1:5000/api/v1/system/health
curl -s http://localhost
serverkit status
```

### 4. Report results

Report clearly:
- Previous version vs new version
- Which branch is deployed
- Backend status (systemd)
- Frontend status (Docker)
- API health check result
- Any errors from logs: `journalctl -u serverkit -n 20 --no-pager`

## On Failure

If any health check fails:
1. Show the last 50 lines of backend logs: `journalctl -u serverkit -n 50 --no-pager`
2. Show frontend Docker logs: `cd /opt/serverkit && docker compose logs --tail=50`
3. Suggest rollback: `serverkit update --branch main --force`
