# Product

## Register

product

## Users

System administrators, DevOps engineers, and technical operators who manage VPS/dedicated servers and the web applications running on them. They reach for ServerKit to do real infrastructure work: deploy and manage apps, databases, Docker containers, DNS, SSL, backups, firewalls, and a fleet of remote agents. Context is focused and consequential — a misclick can take a production service down — so they value precision, predictability, and dense information over hand-holding. Many are comfortable in a terminal and expect keyboard-friendly, fast-loading screens.

## Product Purpose

ServerKit is a server control panel: a single web UI for managing web applications, databases, Docker, networking, and security across one or many servers, with real-time metrics, logs, and a terminal over Socket.IO. It exists to replace the patchwork of SSH sessions, cPanel-style legacy panels, and one-off scripts with a coherent, modern operations surface. Success looks like an operator completing a high-stakes task (deploy, restore a backup, rotate a cert, restart a service) quickly and confidently, trusting that what the UI shows is the true state of the server.

## Brand Personality

Precise and trustworthy. Calm, dense, terminal-adjacent. Confidence comes from clarity and control, not decoration. Voice is direct and technical without jargon-for-jargon's-sake: it names the exact thing that will happen and the exact resource it affects. Three words: **precise, trustworthy, controlled.** The interface should feel like a well-built instrument panel — every readout legible, every control labeled, nothing ambiguous.

## Anti-references

- **Generic AI-SaaS / Linear clone.** No gradient text, glassmorphism, hero-metric cards, identical icon-card grids, or tracked-uppercase eyebrow kickers above every section.
- **Consumer-playful / rounded toy UI.** No oversized rounded blobs, pastel gradients, bouncy/elastic motion, or mascot energy. This manages production infrastructure.
- **Legacy cPanel/WHM clutter.** No dense gray boxes, inconsistent spacing, or 2010-era admin sprawl.
- **Heavy enterprise (SAP/Azure).** No overwhelming nav trees, joyless cramped tables, or density without hierarchy.

## Design Principles

1. **The UI is ground truth.** Show real server state accurately and in real time; never imply success that hasn't happened. Loading, stale, and error states are first-class, not afterthoughts.
2. **Name the consequence.** Buttons and confirmations state the exact action and the exact resource (verb + object), because operations here are hard to undo.
3. **Density with hierarchy.** Pack information for power users, but earn it with clear scale/weight contrast and rhythm so nothing reads as clutter.
4. **Keyboard-first, fast.** Respect operators who live on the keyboard; keep interactions quick and predictable, with visible focus and logical tab order.
5. **Decoration serves legibility.** Color, motion, and emphasis exist to communicate status and guide attention, never to ornament.

## Accessibility & Inclusion

Target WCAG 2.1 AA. Body text ≥4.5:1 contrast in both dark (default) and light themes; large text ≥3:1. Visible keyboard focus on every interactive element with logical tab order; no keyboard traps in modals, drawers, the terminal, or the command palette. Status must never be conveyed by color alone (pair with icon/label) for color-blind operators. Respect `prefers-reduced-motion` for all transitions and real-time animations. Form inputs need associated labels, required indicators, and clear error messaging.
