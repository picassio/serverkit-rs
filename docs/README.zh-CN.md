<div align="center">

# ServerKit

<img width="700" alt="server-kit" src="https://github.com/user-attachments/assets/bfc59e75-8f90-4674-a222-e18213b628b7" />

**自托管基础设施，化繁为简。**

一款轻量、现代的服务器控制面板，用于管理 Web 应用、数据库、
Docker 容器和安全策略——无需 Kubernetes 的复杂性，
也没有托管平台的高昂成本。

[English](../README.md) | [Español](README.es.md) | 中文版 | [Português](README.pt.md)

<br>

![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)
![Docker](https://img.shields.io/badge/Docker-2496ED?style=for-the-badge&logo=docker&logoColor=white)
[![Discord](https://img.shields.io/discord/1470639209059455008?style=for-the-badge&logo=discord&logoColor=white&label=Discord&color=5865F2)](https://discord.gg/ZKk6tkCQfG)

[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](../LICENSE)
[![Python](https://img.shields.io/badge/python-3.11+-3776AB.svg?style=flat-square&logo=python&logoColor=white)](https://python.org)
[![React](https://img.shields.io/badge/react-18-61DAFB.svg?style=flat-square&logo=react&logoColor=black)](https://reactjs.org)
[![Flask](https://img.shields.io/badge/flask-3.0-000000.svg?style=flat-square&logo=flask&logoColor=white)](https://flask.palletsprojects.com)
[![Nginx](https://img.shields.io/badge/nginx-reverse_proxy-009639.svg?style=flat-square&logo=nginx&logoColor=white)](https://nginx.org)
[![Let's Encrypt](https://img.shields.io/badge/SSL-Let's_Encrypt-003A70.svg?style=flat-square&logo=letsencrypt&logoColor=white)](https://letsencrypt.org)

<br>

[功能特性](#-功能特性) · [快速开始](#-快速开始) · [截图预览](#-截图预览) · [系统架构](#-系统架构) · [路线图](#-路线图) · [文档](#-文档) · [参与贡献](#-参与贡献) · [社区](#-社区)

</div>

---

<p align="center">
  <img alt="Dashboard" width="100%" src="https://github.com/user-attachments/assets/e4382ba7-7b3f-4175-baaf-cc9c782de067" />
</p>

---

## 🎯 功能特性

### 应用与容器

🐘 **PHP / WordPress** — PHP-FPM 8.x，支持一键安装 WordPress

🐍 **Python 应用** — 使用 Gunicorn 部署 Flask 和 Django

🟢 **Node.js** — PM2 进程管理，支持日志实时推送

📦 **Manual & Zip Deploys** — 除 Git 和 Docker 源外，还支持从本地/手动路径或上传的 zip 包部署

🐳 **Docker** — 全面的容器和 Docker Compose 管理

♻️ **Container Lifecycle** — 镜像更新检测与一键应用、空闲容器自动休眠，以及基于 CPU 的水平自动扩缩容

🔑 **环境变量** — 安全加密的应用级变量管理

### 基础设施

🌐 **域名管理** — Nginx 虚拟主机，配置简便

🔒 **SSL / TLS** — Let's Encrypt 自动签发与自动续期、可选的（尽力而为）HTTPS（绝不阻断安装）、强化的 TLS 1.2+/AEAD 加密套件、Cloudflare 感知配置，以及自动 CAA 记录

🗄️ **数据库** — 支持 MySQL/MariaDB 和 PostgreSQL

🛡️ **防火墙 (UFW)** — 可视化防火墙规则管理

⏰ **定时任务** — 可视化编辑器调度 Cron 任务

📁 **文件管理器** — 通过 Web 界面浏览和编辑文件，包括浏览和预览 S3 / Backblaze B2 存储桶

📡 **FTP 服务器** — 管理 vsftpd 用户和访问权限

### 安全

🔐 **双因素认证** — 基于 TOTP 的验证，支持备用恢复码

🔑 **Passkeys / WebAuthn** — 无密码登录与二次验证，支持硬件密钥、Touch ID 和 Windows Hello

🛡️ **Web Application Firewall** — 应用级 ModSecurity v3 + OWASP Core Rule Set，支持检测/拦截模式、可调偏执级别，以及审计日志事件

🦠 **恶意软件扫描** — 集成 ClamAV，支持隔离处理

📋 **文件完整性监控** — 检测未授权的文件变更

🚨 **安全告警** — 实时威胁通知

🧱 **Fail2ban 和 SSH** — 暴力破解防护与 SSH 密钥管理

### 监控与告警

📊 **实时指标** — 通过 WebSocket 监控 CPU、内存、磁盘、网络

🎮 **GPU Monitoring** — NVIDIA GPU 利用率、显存、温度和功耗，支持按进程和按容器统计用量

📈 **运行时间追踪** — 服务器历史在线率数据

🔔 **通知推送** — 支持 Discord、Slack、Telegram 及通用 Webhook

🖥️ **多服务器管理** — 基于 Agent 的远程服务器监控与管理

---

## 🚀 快速开始

> ⏱️ 不到 2 分钟即可启动运行

### 方式一：一键安装（推荐）

```bash
curl -fsSL https://serverkit.ai/install.sh | bash
```

> 支持 Ubuntu 22.04+ 和 Debian 12+，自动完成所有配置。

### 方式二：Docker

```bash
git clone https://github.com/jhd3197/ServerKit.git
cd ServerKit
cp .env.example .env       # 编辑 .env 文件，填入你的密钥
docker compose up -d       # 访问 http://localhost
```

### 方式三：手动安装

参阅 [安装指南](INSTALLATION.md) 获取详细的分步说明。

### 系统要求

| | 最低配置 | 推荐配置 |
|---|---------|-------------|
| **操作系统** | Ubuntu 22.04 LTS | Ubuntu 24.04 LTS |
| **CPU** | 1 vCPU | 2+ vCPU |
| **内存** | 1 GB | 2+ GB |
| **磁盘** | 10 GB | 20+ GB |
| **Docker** | 24.0+ | 最新版 |

---

## 📸 截图预览

<p align="center">

![Workflow-Builder](https://github.com/user-attachments/assets/fc58beac-5e2c-487e-a37b-eaa6473eb325)

</p>

<details>
<summary><strong>查看更多截图</strong></summary>

<br>

<p align="center">
  <img alt="Docker" width="100%" src="https://github.com/user-attachments/assets/08fd47b9-778a-4170-8542-ed28579f8a12" />
</p>

<p align="center">
  <img width="100%" alt="Workflow Builder" src="https://github.com/user-attachments/assets/1271f01d-f666-4609-8bc0-22a22c81eaf3" />
</p>

<p align="center">
  <img width="100%" alt="Templates" src="https://github.com/user-attachments/assets/337bcf4a-d5aa-4496-b74a-b66e859304ad" />
</p>

<p align="center">
  <img width="100%" alt="Applications" src="https://github.com/user-attachments/assets/b5bdf80c-4ce1-4de9-b8b9-ae069a17a2b3" />
</p>

<p align="center">
  <img width="100%" alt="Applications Logs"  src="https://github.com/user-attachments/assets/3c397d1e-1452-4111-baae-452fb7bfbed7" />
</p>

</details>

---

## 🏗️ 系统架构

```
                          ┌──────────────────┐
                          │     INTERNET     │
                          └────────┬─────────┘
                                   │
                                   ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                            YOUR SERVER                                    │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                      NGINX (Reverse Proxy)                          │ │
│  │                         :80 / :443                                  │ │
│  │                                                                     │ │
│  │    app1.com ──┐      app2.com ──┐      api.app3.com ──┐            │ │
│  └───────────────┼─────────────────┼─────────────────────┼─────────────┘ │
│                  │ proxy_pass      │ proxy_pass          │ proxy_pass    │
│                  ▼                 ▼                     ▼               │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                      DOCKER CONTAINERS                              │ │
│  │                                                                     │ │
│  │    ┌───────────┐    ┌───────────┐    ┌───────────┐                 │ │
│  │    │ WordPress │    │   Flask   │    │  Node.js  │    ...          │ │
│  │    │   :8001   │    │   :8002   │    │   :8003   │                 │ │
│  │    └─────┬─────┘    └───────────┘    └───────────┘                 │ │
│  └──────────┼──────────────────────────────────────────────────────────┘ │
│             │                                                            │
│             ▼                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                        DATABASES                                    │ │
│  │    MySQL :3306    PostgreSQL :5432    Redis :6379                  │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────┘
```

**[查看完整架构文档 →](ARCHITECTURE.md)** — 请求流程、模板系统、端口分配、数据库关联及故障排查。

---

## 🗺️ 路线图

- [x] 核心基础设施 — Flask + React + JWT + WebSocket
- [x] 应用管理 — PHP、Python、Node.js、Docker
- [x] 域名与 SSL — Nginx 虚拟主机、Let's Encrypt
- [x] 数据库 — MySQL、PostgreSQL
- [x] 文件与 FTP 管理
- [x] 监控与告警 — 指标、Webhook、运行时间追踪
- [x] 安全 — 双因素认证、ClamAV、文件完整性、Fail2ban
- [x] 防火墙 — UFW 集成
- [x] 多服务器管理 — Go Agent、集中式仪表盘
- [x] Git 部署 — Webhook、自动部署、回滚、零停机
- [ ] 备份与恢复 — S3、Backblaze B2、定时备份
- [ ] 邮件服务器 — Postfix、Dovecot、DKIM/SPF/DMARC
- [ ] 团队与权限 — RBAC、审计日志
- [ ] 移动应用 — React Native，支持推送通知
- [ ] 插件市场 — 扩展、自定义组件、主题
- [x] Web 应用防火墙 — 应用级 ModSecurity v3 + OWASP CRS
- [x] 容器安全 — 镜像 CVE 扫描（grype）+ SBOM（syft）
- [x] 无密码认证 — WebAuthn / passkeys
- [x] 动态 DNS — 基于令牌认证的 A/AAAA 更新
- [x] GPU 监控 — NVIDIA 利用率、显存与进程
- [x] 容器生命周期 — 镜像更新应用、自动休眠、水平自动扩缩容
- [x] TLS 强化 — 可选 HTTPS、Cloudflare 感知配置、自动 CAA
- [x] 密钥管理器与入站 Webhook 网关
- [x] 远程访问 — 通过 Agent 托管的 WireGuard 隧道暴露私有/NAT 后的服务
- [x] 连接中心 — 统一管理外部账户（源码、云、DNS、注册商、SMTP、存储）
- [x] WordPress 发布 — 真实子域名、URL 替换、自定义域名、通配符 HTTPS

完整详情：[ROADMAP.md](../ROADMAP.md)

---

## 📖 文档

| 文档 | 说明 |
|----------|-------------|
| [系统架构](ARCHITECTURE.md) | 系统设计、请求流程、架构图 |
| [安装指南](INSTALLATION.md) | 完整的安装配置说明 |
| [部署指南](DEPLOYMENT.md) | CLI 命令与生产环境部署 |
| [API 参考](API.md) | REST API 接口文档 |
| [新功能](NEW_FEATURES.md) | 最新 `dev` 功能的接口与页面参考 |
| [路线图](../ROADMAP.md) | 开发路线图与规划功能 |
| [参与贡献](../CONTRIBUTING.md) | 如何参与贡献 |

---

## 🧱 技术栈

| 层级 | 技术 |
|-------|------------|
| 后端 | Python 3.11, Flask, SQLAlchemy, Flask-SocketIO |
| 前端 | React 18, Vite, SCSS |
| 数据库 | SQLite / PostgreSQL |
| Web 服务器 | Nginx, Gunicorn |
| 容器 | Docker, Docker Compose |
| 安全 | ClamAV, TOTP (pyotp), Cryptography, ModSecurity v3 + OWASP CRS, grype, syft |

---

## 🤝 参与贡献

欢迎贡献代码！请先阅读 [CONTRIBUTING.md](../CONTRIBUTING.md)。

```
Fork → 创建功能分支 → 提交代码 → 推送 → 发起 Pull Request
```

**优先领域：** 备份功能实现、更多通知渠道、UI/UX 改进、文档完善。

---

## 🔭 相关项目

**[Faro](https://github.com/jhd3197/faro)** — 同一作者打造的现代桌面客户端，支持 SFTP、FTP、SSH 和兼容 S3 的存储。保存一次服务器后，即可在双面板视图中浏览其文件，并在同一 SSH 会话上打开终端 — 还支持拖放传输、单向目录同步以及原地编辑。它甚至内置 **Agent Bridge**，让 Claude Code（或任何 MCP 智能体）通过你已认证的会话在服务器上执行命令，逐条命令审批，且无需共享凭据。

> ServerKit 在浏览器中管理你的服务器；Faro 则是桌面端的得力搭档，用于动手进行文件传输、终端操作以及跨服务器的临时任务。[获取构建版本 →](https://github.com/jhd3197/faro/releases/latest)

---

## 💬 社区

[![Discord](https://img.shields.io/badge/Discord-加入我们-5865F2?style=for-the-badge&logo=discord&logoColor=white)](https://discord.gg/ZKk6tkCQfG)

加入 Discord 社区，提出问题、分享反馈，或获取安装部署方面的帮助。

---

<div align="center">

**ServerKit** — 简洁。现代。自托管。

[报告 Bug](https://github.com/jhd3197/ServerKit/issues) · [功能建议](https://github.com/jhd3197/ServerKit/issues)

由 [Juan Denis](https://juandenis.com) 用 ❤️ 打造

</div>
