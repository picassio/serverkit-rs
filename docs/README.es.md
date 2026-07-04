<div align="center">

# ServerKit

<img width="700" alt="server-kit" src="https://github.com/user-attachments/assets/bfc59e75-8f90-4674-a222-e18213b628b7" />

**Infraestructura autoalojada, simplificada.**

Un panel de control de servidores ligero y moderno para gestionar aplicaciones web, bases de datos,
contenedores Docker y seguridad — sin la complejidad de Kubernetes
ni el coste de las plataformas gestionadas.

[English](../README.md) | Español | [中文版](README.zh-CN.md) | [Português](README.pt.md)

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

[Funcionalidades](#-funcionalidades) · [Inicio Rápido](#-inicio-rápido) · [Capturas de Pantalla](#-capturas-de-pantalla) · [Arquitectura](#-arquitectura) · [Hoja de Ruta](#-hoja-de-ruta) · [Documentación](#-documentación) · [Contribuir](#-contribuir) · [Discord](#-comunidad)

</div>

---

<p align="center">
  <img alt="Dashboard" width="100%" src="https://github.com/user-attachments/assets/e4382ba7-7b3f-4175-baaf-cc9c782de067" />
</p>

---

## 🎯 Funcionalidades

### Aplicaciones y Contenedores

🐘 **PHP / WordPress** — PHP-FPM 8.x con instalación de WordPress en un clic

🐍 **Aplicaciones Python** — Despliega Flask y Django con Gunicorn

🟢 **Node.js** — Aplicaciones gestionadas con PM2 y transmisión de logs en tiempo real

📦 **Despliegues Manuales y Zip** — Despliega desde una ruta local/manual o subiendo un zip, junto a los orígenes Git y Docker

🐳 **Docker** — Gestión completa de contenedores y Docker Compose

♻️ **Ciclo de Vida de Contenedores** — Detección de actualizaciones de imagen con aplicación en un clic, suspensión automática de contenedores inactivos y escalado horizontal automático según la CPU

🔑 **Variables de Entorno** — Gestión segura y cifrada de variables por aplicación

### Infraestructura

🌐 **Gestión de Dominios** — Hosts virtuales Nginx con configuración sencilla

🔒 **SSL / TLS** — Let's Encrypt automático con renovación automática, HTTPS opcional (en la medida de lo posible) que nunca bloquea una instalación, cifrados reforzados TLS 1.2+/AEAD, configuraciones compatibles con Cloudflare y registros CAA automáticos

🗄️ **Bases de Datos** — Soporte para MySQL/MariaDB y PostgreSQL

🛡️ **Firewall (UFW)** — Gestión visual de reglas de firewall

⏰ **Tareas Programadas (Cron)** — Programa tareas con un editor visual

📁 **Gestor de Archivos** — Navega y edita archivos desde la interfaz web, incluyendo la navegación y vista previa de buckets de S3 / Backblaze B2

📡 **Servidor FTP** — Gestión de usuarios y accesos de vsftpd

### Seguridad

🔐 **Autenticación de Dos Factores** — Basada en TOTP con códigos de respaldo

🔑 **Passkeys / WebAuthn** — Inicio de sesión sin contraseña y como segundo factor con llaves de hardware, Touch ID y Windows Hello

🧱 **Firewall de Aplicaciones Web** — ModSecurity v3 + OWASP Core Rule Set por aplicación con modos de detección/bloqueo, paranoia ajustable y eventos en el registro de auditoría

🦠 **Análisis de Malware** — Integración con ClamAV y cuarentena

📋 **Monitorización de Integridad de Archivos** — Detecta cambios no autorizados en archivos

🚨 **Alertas de Seguridad** — Notificaciones de amenazas en tiempo real

🧱 **Fail2ban y SSH** — Protección contra fuerza bruta y gestión de claves SSH

### Monitorización y Alertas

📊 **Métricas en Tiempo Real** — Monitorización de CPU, RAM, disco y red vía WebSocket

🎮 **Monitorización de GPU** — Utilización, memoria, temperatura y consumo de GPU NVIDIA, con uso por proceso y por contenedor

📈 **Seguimiento de Disponibilidad** — Datos históricos de tiempo de actividad del servidor

🔔 **Notificaciones** — Discord, Slack, Telegram y webhooks genéricos

🖥️ **Multi-Servidor** — Monitorización y gestión remota de servidores basada en agentes

---

## 🚀 Inicio Rápido

> ⏱️ En funcionamiento en menos de 2 minutos

### Opción 1: Instalación en Una Línea (Recomendada)

```bash
curl -fsSL https://serverkit.ai/install.sh | bash
```

> Funciona en Ubuntu 22.04+ y Debian 12+. Configura todo automáticamente.

### Opción 2: Docker

```bash
git clone https://github.com/jhd3197/ServerKit.git
cd ServerKit
cp .env.example .env       # luego edita .env con tus claves
docker compose up -d       # accede en http://localhost
```

### Opción 3: Instalación Manual

Consulta la [Guía de Instalación](INSTALLATION.md) para instrucciones paso a paso.

### Requisitos

| | Mínimo | Recomendado |
|---|---------|-------------|
| **SO** | Ubuntu 22.04 LTS | Ubuntu 24.04 LTS |
| **CPU** | 1 vCPU | 2+ vCPU |
| **RAM** | 1 GB | 2+ GB |
| **Disco** | 10 GB | 20+ GB |
| **Docker** | 24.0+ | Última versión |

---

## 📸 Capturas de Pantalla

<p align="center">

![Workflow-Builder](https://github.com/user-attachments/assets/fc58beac-5e2c-487e-a37b-eaa6473eb325)

</p>

<details>
<summary><strong>Ver Más Capturas</strong></summary>

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

## 🏗️ Arquitectura

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

**[Ver Documentación Completa de Arquitectura →](ARCHITECTURE.md)** — Flujo de peticiones, sistema de plantillas, asignación de puertos, vinculación de bases de datos y resolución de problemas.

---

## 🗺️ Hoja de Ruta

- [x] Infraestructura base — Flask + React + JWT + WebSocket
- [x] Gestión de aplicaciones — PHP, Python, Node.js, Docker
- [x] Dominios y SSL — Hosts virtuales Nginx, Let's Encrypt
- [x] Bases de datos — MySQL, PostgreSQL
- [x] Gestión de archivos y FTP
- [x] Monitorización y alertas — Métricas, webhooks, seguimiento de disponibilidad
- [x] Seguridad — 2FA, ClamAV, integridad de archivos, Fail2ban
- [x] Firewall — Integración con UFW
- [x] Gestión multi-servidor — Agente en Go, panel centralizado
- [x] Despliegue con Git — Webhooks, despliegue automático, rollback, sin tiempo de inactividad
- [ ] Copias de seguridad y restauración — S3, Backblaze B2, copias programadas
- [ ] Servidor de correo — Postfix, Dovecot, DKIM/SPF/DMARC
- [ ] Equipos y permisos — RBAC, registro de auditoría
- [ ] Aplicación móvil — React Native con notificaciones push
- [ ] Marketplace de plugins — Extensiones, widgets personalizados, temas
- [x] Firewall de Aplicaciones Web — ModSecurity v3 + OWASP CRS por aplicación
- [x] Seguridad de contenedores — Análisis de CVE de imágenes (grype) + SBOM (syft)
- [x] Autenticación sin contraseña — WebAuthn / passkeys
- [x] DNS dinámico — Actualizaciones A/AAAA autenticadas por token
- [x] Monitorización de GPU — Utilización, memoria y procesos de NVIDIA
- [x] Ciclo de vida de contenedores — Aplicación de actualizaciones de imagen, suspensión automática, escalado horizontal automático
- [x] Refuerzo de TLS — HTTPS opcional, configuraciones compatibles con Cloudflare, CAA automático
- [x] Gestor de secretos y pasarela de webhooks entrantes
- [x] Acceso remoto — Expón servicios privados/tras NAT mediante túneles WireGuard gestionados por el agente
- [x] Centro de conexiones — Cuentas externas unificadas (origen, nube, DNS, registradores, SMTP, almacenamiento)
- [x] Publicación de WordPress — Subdominios reales, intercambio de URL, dominios personalizados, HTTPS comodín

Detalles completos: [ROADMAP.md](../ROADMAP.md)

---

## 📖 Documentación

| Documento | Descripción |
|----------|-------------|
| [Arquitectura](ARCHITECTURE.md) | Diseño del sistema, flujo de peticiones, diagramas |
| [Guía de Instalación](INSTALLATION.md) | Instrucciones completas de configuración |
| [Guía de Despliegue](DEPLOYMENT.md) | Comandos CLI y despliegue en producción |
| [Referencia de la API](API.md) | Endpoints de la API REST |
| [Nuevas Funcionalidades](NEW_FEATURES.md) | Referencia de endpoints y páginas de las últimas funcionalidades de `dev` |
| [Hoja de Ruta](../ROADMAP.md) | Hoja de ruta de desarrollo y funcionalidades planificadas |
| [Contribuir](../CONTRIBUTING.md) | Cómo contribuir |

---

## 🧱 Stack Tecnológico

| Capa | Tecnología |
|-------|------------|
| Backend | Python 3.11, Flask, SQLAlchemy, Flask-SocketIO |
| Frontend | React 18, Vite, SCSS |
| Base de Datos | SQLite / PostgreSQL |
| Servidor Web | Nginx, Gunicorn |
| Contenedores | Docker, Docker Compose |
| Seguridad | ClamAV, ModSecurity v3 + OWASP CRS, grype, syft, TOTP (pyotp), Cryptography |

---

## 🤝 Contribuir

¡Las contribuciones son bienvenidas! Por favor, lee primero [CONTRIBUTING.md](../CONTRIBUTING.md).

```
fork → rama de funcionalidad → commit → push → pull request
```

**Áreas prioritarias:** Implementación de copias de seguridad, canales de notificación adicionales, mejoras de UI/UX, documentación.

---

## 🔭 Proyectos Relacionados

**[Faro](https://github.com/jhd3197/faro)** — Un cliente de escritorio moderno para SFTP, FTP, SSH y almacenamiento compatible con S3, del mismo autor. Guarda un servidor una vez y luego explora sus archivos en una vista de doble panel y abre una terminal sobre la misma sesión SSH — además de transferencias con arrastrar y soltar, sincronización de directorios en un sentido y edición in situ. Incluso tiene un **Agent Bridge** que permite a Claude Code (o cualquier agente MCP) ejecutar comandos en un servidor a través de tu sesión autenticada, con aprobación por comando y sin compartir credenciales.

> ServerKit gestiona tus servidores desde el navegador; Faro es el compañero de escritorio para transferencias de archivos, shells y trabajo puntual en todos tus servidores. [Descarga una versión →](https://github.com/jhd3197/faro/releases/latest)

---

## 💬 Comunidad

[![Discord](https://img.shields.io/badge/Discord-Únete-5865F2?style=for-the-badge&logo=discord&logoColor=white)](https://discord.gg/ZKk6tkCQfG)

Únete al Discord para hacer preguntas, compartir comentarios u obtener ayuda con tu configuración.

---

<div align="center">

**ServerKit** — Simple. Moderno. Autoalojado.

[Reportar un Error](https://github.com/jhd3197/ServerKit/issues) · [Solicitar una Funcionalidad](https://github.com/jhd3197/ServerKit/issues)

Hecho con ❤️ por [Juan Denis](https://juandenis.com)

</div>
