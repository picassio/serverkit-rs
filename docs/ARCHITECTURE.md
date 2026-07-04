# ServerKit Architecture

> Deep dive into how ServerKit connects domains, apps, containers, and databases.

---

## Table of Contents

- [System Overview](#system-overview)
- [Request Flow](#request-flow)
- [Template System](#template-system)
- [Port Allocation](#port-allocation)
- [Database Linking](#database-linking)
- [Workflow Automation](#workflow-automation)
- [Environment Pipeline](#environment-pipeline)
- [File Paths](#file-paths)

---

## System Overview

```
                                    ┌─────────────────────────────────────────────────────────────┐
                                    │                        INTERNET                             │
                                    └─────────────────────────────────────────────────────────────┘
                                                              │
                                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                         YOUR SERVER                                                  │
│  ┌───────────────────────────────────────────────────────────────────────────────────────────────┐  │
│  │                                    NGINX (Reverse Proxy)                                      │  │
│  │                                      Port 80 / 443                                            │  │
│  │                                                                                               │  │
│  │   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐              │  │
│  │   │ app1.com     │    │ app2.com     │    │ api.app3.com │    │ Private URLs │              │  │
│  │   │    :443      │    │    :443      │    │    :443      │    │  /p/abc123   │              │  │
│  │   └──────┬───────┘    └──────┬───────┘    └──────┬───────┘    └──────┬───────┘              │  │
│  └──────────┼───────────────────┼───────────────────┼───────────────────┼────────────────────────┘  │
│             │                   │                   │                   │                           │
│             │ proxy_pass        │ proxy_pass        │ proxy_pass        │ proxy_pass                │
│             ▼                   ▼                   ▼                   ▼                           │
│  ┌─────────────────────────────────────────────────────────────────────────────────────────────┐   │
│  │                                    DOCKER CONTAINERS                                         │   │
│  │                                                                                              │   │
│  │   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐             │   │
│  │   │  WordPress   │    │    Flask     │    │   Node.js    │    │   Custom     │             │   │
│  │   │  Port 8001   │    │  Port 8002   │    │  Port 8003   │    │  Port 8004   │             │   │
│  │   │              │    │              │    │              │    │              │             │   │
│  │   │ ┌──────────┐ │    │ ┌──────────┐ │    │ ┌──────────┐ │    │ ┌──────────┐ │             │   │
│  │   │ │ Apache   │ │    │ │ Gunicorn │ │    │ │   PM2    │ │    │ │  Your    │ │             │   │
│  │   │ │ PHP-FPM  │ │    │ │ Python   │ │    │ │ Express  │ │    │ │  App     │ │             │   │
│  │   │ └──────────┘ │    │ └──────────┘ │    │ └──────────┘ │    │ └──────────┘ │             │   │
│  │   └──────┬───────┘    └──────────────┘    └──────────────┘    └──────────────┘             │   │
│  │          │                                                                                  │   │
│  └──────────┼──────────────────────────────────────────────────────────────────────────────────┘   │
│             │                                                                                       │
│             ▼                                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────────────────────────────┐   │
│  │                                      DATABASES                                               │   │
│  │                                                                                              │   │
│  │   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐             │   │
│  │   │    MySQL     │    │  PostgreSQL  │    │    Redis     │    │   MongoDB    │             │   │
│  │   │  Port 3306   │    │  Port 5432   │    │  Port 6379   │    │  Port 27017  │             │   │
│  │   └──────────────┘    └──────────────┘    └──────────────┘    └──────────────┘             │   │
│  │                                                                                              │   │
│  └──────────────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Request Flow

What happens when a user visits your app:

```
User Request                    What Happens
─────────────────────────────────────────────────────────────────────────────────

  Browser                 1. DNS resolves app1.com to your server IP
     │
     ▼
┌─────────┐              2. Request hits Nginx on port 80/443
│  Nginx  │                 Nginx checks server_name directives
│ :80/443 │                 Matches "app1.com" → proxy_pass http://127.0.0.1:8001
└────┬────┘
     │
     ▼
┌─────────┐              3. Nginx forwards request to Docker container
│ Docker  │                 Container receives request on internal port
│ :8001   │                 App processes and returns response
└────┬────┘
     │
     ▼
┌─────────┐              4. Response flows back through Nginx
│ Response│                 SSL termination handled by Nginx
│  200 OK │                 User sees the page
└─────────┘
```

---

## Template System

### Template → App → Domain Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              TEMPLATE LIBRARY                                    │
│                           /etc/serverkit/templates/                              │
│                                                                                  │
│   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐             │
│   │WordPress│  │  Flask  │  │ Node.js │  │ Grafana │  │  n8n    │  ... 60+    │
│   │  .yaml  │  │  .yaml  │  │  .yaml  │  │  .yaml  │  │  .yaml  │             │
│   └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘             │
│        │            │            │            │            │                   │
└────────┼────────────┼────────────┼────────────┼────────────┼───────────────────┘
```

---

## Port Allocation

### How ServerKit Finds Available Ports

ServerKit automatically scans the database, Docker, and system sockets to find the first available port (starting from 8000) for new applications, ensuring no conflicts occur during deployment.

---

## Database Linking

### How Apps Connect to Databases

ServerKit automates the creation of databases and users, then injects the credentials as environment variables (`DB_HOST`, `DB_USER`, etc.) directly into the application container, allowing for seamless connectivity.

---

## Workflow Automation

ServerKit includes a node-based visual workflow builder for automating server tasks.

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              WORKFLOW BUILDER                                    │
│                                                                                  │
│   ┌───────────┐       ┌───────────┐       ┌───────────┐       ┌───────────┐      │
│   │  TRIGGER  │──────▶│  ACTION   │──────▶│ CONDITION │──────▶│  NOTIFY   │      │
│   │ (Git Push)│       │ (Build)   │       │ (Success?)│       │ (Discord) │      │
│   └───────────┘       └───────────┘       └─────┬─────┘       └───────────┘      │
│                                                 │                                │
│                                                 ▼                                │
│                                           ┌───────────┐                          │
│                                           │  ROLLBACK │                          │
│                                           └───────────┘                          │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

- **Nodes:** Represent individual steps (Triggers, Actions, Logic, Notifications).
- **Edges:** Define the flow of execution.
- **Engine:** The `WorkflowService` parses the JSON graph and executes steps sequentially or in parallel.

---

## Environment Pipeline

Specifically designed for WordPress, the environment pipeline allows for professional staging/dev workflows.

```
┌──────────────┐ promotion  ┌──────────────┐ promotion  ┌──────────────┐
│     DEV      │───────────▶│   STAGING    │───────────▶│  PRODUCTION  │
│ (Standalone) │            │ (Standalone) │            │ (Production) │
└──────┬───────┘            └──────┬───────┘            └──────┬───────┘
       │                           │                           │
       └─────────── sync ──────────┴─────────── sync ──────────┘
```

- **Promotion:** Push code (Git) and Database from a lower environment to a higher one.
- **Syncing:** Pull the latest production database and media to dev/staging for testing.
- **Sanitization:** Automatically strip sensitive user data during sync.

---

## File Paths

### Where Everything Lives

```
/var/serverkit/                          # ServerKit data root
├── apps/                                # All deployed applications
├── backups/                             # Database backups
└── ssl/                                 # SSL certificates

/etc/serverkit/                          # ServerKit configuration
├── templates/                           # Template library (YAML files)
└── config.yaml                          # Main config
```

---

## Component Diagram

ServerKit follows a modern 3-tier architecture:
1. **Frontend:** React-based dashboard served via Nginx.
2. **Backend:** Flask REST API managing Docker, Nginx, and system services.
3. **Agent:** Go-based remote agent for multi-server management.

> **Deployment constraint — single WebSocket worker.** The agent gateway keeps
> all connected-agent state (the registry of live agents, socket↔server index,
> session tokens, and in-flight command queues) **in-memory in one process**.
> Run the panel with a **single** gunicorn worker process — a plain threaded
> worker (`-w 1 --threads N`; WebSocket is served by simple-websocket to match
> `async_mode='threading'` — the gevent-websocket worker class double-answers
> the WS handshake and breaks WebSocket). Scaling to multiple workers without
> a shared backplane (e.g. a Redis message queue) will silently misroute or
> drop commands for agents connected to a different worker.

---

## Troubleshooting

Refer to the [Deployment Guide](DEPLOYMENT.md) for detailed troubleshooting steps regarding 502 errors, container failures, and networking issues.

---

## See Also

- [Installation Guide](INSTALLATION.md)
- [API Reference](API.md)
- [Deployment Guide](DEPLOYMENT.md)
