"""Brand tokens for rendered notifications.

Single source of truth for the colors/wordmark used by email templates (and,
later, any other rich channel). Mirrors the frontend SCSS design system
(`frontend/src/styles/_variables.scss`) so email matches the app:

    accent  #6d7cff   success #3ddc97   warning #f5b945   danger #fb6f6f

The severity *tints* below are tuned for legibility on a light email canvas
(emails render light-first; a dark-mode media query swaps the chrome).
"""

BRAND = {
    'name': 'ServerKit',
    'accent': '#6d7cff',
    'accent_hover': '#5a67e8',
    # Light canvas
    'page_bg': '#eef0f6',
    'card_bg': '#ffffff',
    'border': '#e6e8f0',
    'ink': '#1b1e2b',       # primary text
    'muted': '#646b7d',     # secondary text
    'faint': '#9aa1b2',     # footer / least important
    'code_bg': '#f3f4f9',   # monospace value chips
    # Dark canvas (used by the prefers-color-scheme media query)
    'dark_page_bg': '#0f1117',
    'dark_card_bg': '#171a23',
    'dark_border': '#272b38',
    'dark_ink': '#e9ebf2',
    'dark_muted': '#9aa1b2',
    'dark_code_bg': '#1f2330',
}

# Per-severity styling: a light tint background, a readable text color, and a
# saturated dot — all solid hex so they survive every email client.
SEVERITY_STYLES = {
    'critical': {'bg': '#fdecec', 'text': '#c0392b', 'dot': '#fb6f6f', 'label': 'Critical'},
    'warning':  {'bg': '#fdf5e3', 'text': '#9a6a12', 'dot': '#f5b945', 'label': 'Warning'},
    'info':     {'bg': '#eef0ff', 'text': '#454dc4', 'dot': '#6d7cff', 'label': 'Info'},
    'success':  {'bg': '#e7f9f1', 'text': '#11784f', 'dot': '#3ddc97', 'label': 'Success'},
    'test':     {'bg': '#f1f2f6', 'text': '#5b6170', 'dot': '#9aa1b2', 'label': 'Test'},
}


def style_for(severity):
    """Return the tint/text/dot triple for a severity (falls back to info)."""
    return SEVERITY_STYLES.get(severity, SEVERITY_STYLES['info'])
