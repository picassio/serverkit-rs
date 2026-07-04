<div align="center">

# ServerKit

<img width="700" alt="server-kit" src="https://github.com/user-attachments/assets/bfc59e75-8f90-4674-a222-e18213b628b7" />

**Infraestrutura auto-hospedada, de forma simples.**

Um painel de controle de servidores leve e moderno para gerenciar aplicações web, bancos de dados,
containers Docker e segurança — sem a complexidade do Kubernetes
ou o custo de plataformas gerenciadas.

[English](../README.md) | [Español](README.es.md) | [中文版](README.zh-CN.md) | Português

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

[Funcionalidades](#-funcionalidades) · [Início Rápido](#-início-rápido) · [Capturas de Tela](#-capturas-de-tela) · [Arquitetura](#-arquitetura) · [Roadmap](#-roadmap) · [Documentação](#-documentação) · [Contribuindo](#-contribuindo) · [Discord](#-comunidade)

</div>

---

<p align="center">
  <img alt="Dashboard" width="100%" src="https://github.com/user-attachments/assets/e4382ba7-7b3f-4175-baaf-cc9c782de067" />
</p>

---

## 🎯 Funcionalidades

### Aplicações & Containers

🐘 **PHP / WordPress** — PHP-FPM 8.x com instalação do WordPress em um clique

🐍 **Aplicações Python** — Deploy de Flask e Django com Gunicorn

🟢 **Node.js** — Aplicações gerenciadas pelo PM2 com streaming de logs

📦 **Deploys Manuais & Zip** — Faça deploy a partir de um caminho local/manual ou de um upload zip, além das fontes Git e Docker

🐳 **Docker** — Gerenciamento completo de containers e Docker Compose

🔄 **Ciclo de Vida de Containers** — Detecção de atualizações de imagem com aplicação em um clique, suspensão automática de containers ociosos e auto-escalonamento horizontal orientado por CPU

🔑 **Variáveis de Ambiente** — Gerenciamento seguro e criptografado por aplicação

### Infraestrutura

🌐 **Gerenciamento de Domínios** — Virtual hosts Nginx com configuração simplificada

🔒 **SSL / TLS** — Let's Encrypt automático com renovação automática, HTTPS opcional (de melhor esforço) que nunca bloqueia uma instalação, cifras TLS 1.2+/AEAD reforçadas, configurações compatíveis com Cloudflare e registros CAA automáticos

🗄️ **Bancos de Dados** — Suporte a MySQL/MariaDB e PostgreSQL

🛡️ **Firewall (UFW)** — Gerenciamento visual de regras de firewall

⏰ **Cron Jobs** — Agendamento de tarefas com editor visual

📁 **Gerenciador de Arquivos** — Navegue e edite arquivos pela interface web, incluindo navegar e visualizar buckets S3 / Backblaze B2

📡 **Servidor FTP** — Gerencie usuários e acessos do vsftpd

### Segurança

🔐 **Autenticação de Dois Fatores** — Baseada em TOTP com códigos de recuperação

🔑 **Passkeys / WebAuthn** — Login sem senha e como segundo fator com chaves de hardware, Touch ID e Windows Hello

🧯 **Firewall de Aplicações Web** — ModSecurity v3 + OWASP Core Rule Set por aplicação, com modos de detecção/bloqueio, paranoia ajustável e eventos no log de auditoria

🦠 **Varredura de Malware** — Integração com ClamAV e quarentena

📋 **Monitoramento de Integridade de Arquivos** — Detecção de alterações não autorizadas

🚨 **Alertas de Segurança** — Notificações de ameaças em tempo real

🧱 **Fail2ban & SSH** — Proteção contra força bruta e gerenciamento de chaves SSH

### Monitoramento & Alertas

📊 **Métricas em Tempo Real** — Monitoramento de CPU, RAM, disco e rede via WebSocket

🎮 **Monitoramento de GPU** — Utilização, memória, temperatura e consumo de GPUs NVIDIA, com uso por processo e por container

📈 **Rastreamento de Uptime** — Histórico de disponibilidade do servidor

🔔 **Notificações** — Discord, Slack, Telegram e webhooks genéricos

🖥️ **Multi-Servidor** — Monitoramento e gerenciamento remoto de servidores baseado em agentes

---

## 🚀 Início Rápido

> ⏱️ Pronto para usar em menos de 2 minutos

### Opção 1: Instalação em Uma Linha (Recomendado)

```bash
curl -fsSL https://serverkit.ai/install.sh | bash
```

> Funciona no Ubuntu 22.04+ e Debian 12+. Configura tudo automaticamente.

### Opção 2: Docker

```bash
git clone https://github.com/jhd3197/ServerKit.git
cd ServerKit
cp .env.example .env       # depois edite o .env com suas chaves secretas
docker compose up -d       # acesse em http://localhost
```

### Opção 3: Instalação Manual

Consulte o [Guia de Instalação](INSTALLATION.md) para instruções passo a passo.

### Requisitos

| | Mínimo | Recomendado |
|---|---------|-------------|
| **SO** | Ubuntu 22.04 LTS | Ubuntu 24.04 LTS |
| **CPU** | 1 vCPU | 2+ vCPU |
| **RAM** | 1 GB | 2+ GB |
| **Disco** | 10 GB | 20+ GB |
| **Docker** | 24.0+ | Mais recente |

---

## 📸 Capturas de Tela

<p align="center">

![Workflow-Builder](https://github.com/user-attachments/assets/fc58beac-5e2c-487e-a37b-eaa6473eb325)

</p>

<details>
<summary><strong>Ver Mais Capturas de Tela</strong></summary>

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

## 🏗️ Arquitetura

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

**[Ver Documentação Completa da Arquitetura →](ARCHITECTURE.md)** — Fluxo de requisições, sistema de templates, alocação de portas, vinculação de bancos de dados e solução de problemas.

---

## 🗺️ Roadmap

- [x] Infraestrutura base — Flask + React + JWT + WebSocket
- [x] Gerenciamento de aplicações — PHP, Python, Node.js, Docker
- [x] Domínio & SSL — Virtual hosts Nginx, Let's Encrypt
- [x] Bancos de dados — MySQL, PostgreSQL
- [x] Gerenciamento de arquivos & FTP
- [x] Monitoramento & alertas — Métricas, webhooks, rastreamento de uptime
- [x] Segurança — 2FA, ClamAV, integridade de arquivos, Fail2ban
- [x] Firewall — Integração com UFW
- [x] Gerenciamento multi-servidor — Agente em Go, painel centralizado
- [x] Deploy via Git — Webhooks, deploy automático, rollback, zero-downtime
- [ ] Backup & restauração — S3, Backblaze B2, backups agendados
- [ ] Servidor de e-mail — Postfix, Dovecot, DKIM/SPF/DMARC
- [ ] Equipes & permissões — RBAC, log de auditoria
- [ ] Aplicativo mobile — React Native com notificações push
- [ ] Marketplace de plugins — Extensões, widgets personalizados, temas
- [x] Firewall de aplicações web — ModSecurity v3 + OWASP CRS por aplicação
- [x] Segurança de containers — Varredura de CVEs de imagem (grype) + SBOM (syft)
- [x] Autenticação sem senha — WebAuthn / passkeys
- [x] DNS dinâmico — Atualizações A/AAAA autenticadas por token
- [x] Monitoramento de GPU — Utilização, memória e processos NVIDIA
- [x] Ciclo de vida de containers — Aplicação de atualização de imagem, suspensão automática, auto-escalonamento horizontal
- [x] Reforço de TLS — HTTPS opcional, configurações compatíveis com Cloudflare, CAA automático
- [x] Gerenciador de secrets & gateway de webhooks de entrada
- [x] Acesso remoto — Exponha serviços privados/NAT via túneis WireGuard gerenciados por agente
- [x] Hub de conexões — Contas externas unificadas (fonte, nuvem, DNS, registradores, SMTP, armazenamento)
- [x] Publicação WordPress — Subdomínios reais, troca de URL, domínios personalizados, HTTPS wildcard

Detalhes completos: [ROADMAP.md](../ROADMAP.md)

---

## 📖 Documentação

| Documento | Descrição |
|----------|-------------|
| [Arquitetura](ARCHITECTURE.md) | Design do sistema, fluxo de requisições, diagramas |
| [Guia de Instalação](INSTALLATION.md) | Instruções completas de configuração |
| [Guia de Deploy](DEPLOYMENT.md) | Comandos CLI e deploy em produção |
| [Referência da API](API.md) | Endpoints da API REST |
| [Novas Funcionalidades](NEW_FEATURES.md) | Referência de endpoints e páginas das funcionalidades mais recentes de `dev` |
| [Roadmap](../ROADMAP.md) | Roadmap de desenvolvimento e funcionalidades planejadas |
| [Contribuindo](../CONTRIBUTING.md) | Como contribuir |

---

## 🧱 Stack Tecnológica

| Camada | Tecnologia |
|-------|------------|
| Backend | Python 3.11, Flask, SQLAlchemy, Flask-SocketIO |
| Frontend | React 18, Vite, SCSS |
| Banco de Dados | SQLite / PostgreSQL |
| Servidor Web | Nginx, Gunicorn |
| Containers | Docker, Docker Compose |
| Segurança | ClamAV, TOTP (pyotp), Cryptography, ModSecurity v3 + OWASP CRS, grype, syft |

---

## 🤝 Contribuindo

Contribuições são bem-vindas! Por favor, leia o [CONTRIBUTING.md](../CONTRIBUTING.md) primeiro.

```
fork → branch de feature → commit → push → pull request
```

**Áreas prioritárias:** Implementações de backup, canais de notificação adicionais, melhorias de UI/UX, documentação.

---

## 🔭 Projetos Relacionados

**[Faro](https://github.com/jhd3197/faro)** — Um cliente de desktop moderno para SFTP, FTP, SSH e armazenamento compatível com S3, do mesmo autor. Salve um servidor uma vez e depois navegue pelos seus arquivos em uma visão de painel duplo e abra um terminal sobre a mesma sessão SSH — além de transferências com arrastar e soltar, sincronização de diretórios em um sentido e edição no lugar. Ele ainda tem um **Agent Bridge** que permite ao Claude Code (ou qualquer agente MCP) executar comandos em um servidor através da sua sessão autenticada, com aprovação por comando e sem compartilhar credenciais.

> O ServerKit gerencia seus servidores pelo navegador; o Faro é o companheiro de desktop para transferência de arquivos, shells e trabalho pontual em todos os seus servidores. [Baixe uma versão →](https://github.com/jhd3197/faro/releases/latest)

---

## 💬 Comunidade

[![Discord](https://img.shields.io/badge/Discord-Junte--se_a_Nós-5865F2?style=for-the-badge&logo=discord&logoColor=white)](https://discord.gg/ZKk6tkCQfG)

Entre no Discord para tirar dúvidas, compartilhar feedback ou obter ajuda com sua configuração.

---

<div align="center">

**ServerKit** — Simples. Moderno. Auto-hospedado.

[Reportar Bug](https://github.com/jhd3197/ServerKit/issues) · [Solicitar Funcionalidade](https://github.com/jhd3197/ServerKit/issues)

Feito com ❤️ por [Juan Denis](https://juandenis.com)

</div>
