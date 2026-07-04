"""
Connection string codec.

A connection string is a single pasteable token the user copies from
the panel and pastes into the agent's pairing wizard. It bundles the
panel host + a single-use registration token + an optional expiry,
replacing the older flow where the user typed the URL and token into
separate fields.

Format: ``sk1://<host>[:<port>]/<token>[?exp=<ISO8601>][&insecure=1]``

The ``sk1://`` scheme self-identifies the format (you can recognise a
ServerKit connection string at a glance) and gives us a version lever:
any future format change goes out as ``sk2://``. The host is visible up
front so the user can sanity-check which panel they're pointing at
before pasting. ``insecure=1`` flips the implied scheme from https to
http for dev / local-network use; absent it, https is implied.

Note: the panel never *reads* connection strings — the agent does.
``decode`` is exported only for tests and for symmetry, so the codec
has a single owner.
"""

from datetime import datetime
from typing import Optional
from urllib.parse import urlparse, parse_qs


SCHEME_PREFIX = "sk1://"


def encode(url: str, token: str, expires_at: Optional[datetime]) -> str:
    """Pack panel URL + token (+ optional expiry) into a single
    ``sk1://<host>/<token>[?…]`` string.

    The url's scheme (http vs https) is preserved via the ``insecure``
    query param: omitted means https, ``insecure=1`` means http. We
    only allow http/https because those are the only schemes agents
    speak; a typo'd input fails loudly here rather than producing a
    broken connection string.
    """
    if not url or not token:
        raise ValueError("url and token are required")
    if "/" in token:
        raise ValueError("token must not contain '/'")

    parsed = urlparse(url)
    if not parsed.scheme or not parsed.netloc:
        raise ValueError(f"url must include scheme and host: {url!r}")
    if parsed.scheme not in ("http", "https"):
        raise ValueError(f"url scheme must be http or https: {parsed.scheme!r}")

    host = parsed.netloc

    # Build the query manually to keep ISO timestamps human-readable —
    # urlencode would percent-encode the colons in `2126-04-10T02:27:24Z`,
    # which is correct but uglier and harder to eyeball.
    query_parts = []
    if expires_at is not None:
        query_parts.append(f"exp={expires_at.isoformat()}Z")
    if parsed.scheme == "http":
        query_parts.append("insecure=1")
    suffix = ("?" + "&".join(query_parts)) if query_parts else ""

    return f"{SCHEME_PREFIX}{host}/{token}{suffix}"


def decode(s: str) -> dict:
    """Reverse of encode. Raises ValueError on any parse error.

    Returns a dict with keys ``url`` (reconstructed http/https URL),
    ``token``, ``expires_at`` (datetime or None).
    """
    if not isinstance(s, str):
        raise ValueError("connection string must be a string")
    s = s.strip()
    if not s:
        raise ValueError("connection string is empty")
    if not s.startswith(SCHEME_PREFIX):
        raise ValueError(
            f"not a sk1 connection string (expected prefix {SCHEME_PREFIX!r})"
        )

    parsed = urlparse(s)
    if parsed.scheme != "sk1":
        raise ValueError(f"unexpected scheme: {parsed.scheme!r}")
    if not parsed.netloc:
        raise ValueError("missing host")
    if not parsed.path or parsed.path == "/":
        raise ValueError("missing token")

    token = parsed.path.lstrip("/")
    # Defensive: a stray slash in the path means the user mangled the
    # string or we somehow encoded a token containing a slash. Either
    # way the safe move is to fail rather than silently truncate.
    if "/" in token:
        raise ValueError("token must not contain '/'")

    qs = parse_qs(parsed.query)
    insecure_raw = qs.get("insecure", [""])[0]
    scheme = "http" if insecure_raw in ("1", "true", "yes") else "https"
    url = f"{scheme}://{parsed.netloc}"

    expires_at: Optional[datetime] = None
    exp_raw = qs.get("exp", [""])[0]
    if exp_raw:
        # Tolerate trailing Z (UTC marker) which datetime.fromisoformat
        # didn't accept until Python 3.11.
        cleaned = exp_raw[:-1] if exp_raw.endswith("Z") else exp_raw
        try:
            expires_at = datetime.fromisoformat(cleaned)
        except Exception as exc:
            raise ValueError(f"invalid exp: {exc}") from exc

    return {"url": url, "token": token, "expires_at": expires_at}
