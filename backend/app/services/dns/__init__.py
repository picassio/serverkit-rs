"""Shared, provider-agnostic DNS client layer.

Both ``DNSProviderService`` (provider/credentials layer) and ``DNSZoneService``
(zone/records layer) build a :class:`DnsCredential` + :class:`DnsRecordSpec` and
talk to a provider through :func:`get_client`, so the API wire format for a given
provider lives in exactly one module.
"""
from app.services.dns.base import DnsCredential, DnsRecordSpec
from app.services.dns.cloudflare import CloudflareClient

__all__ = ['DnsCredential', 'DnsRecordSpec', 'CloudflareClient', 'get_client']


def get_client(credential: DnsCredential):
    """Return the API client for ``credential.provider``.

    Only Cloudflare is shared today (it was the duplicated path); Route53 /
    DigitalOcean / GoDaddy still live in ``DNSProviderService`` and can move
    behind this factory later without changing callers.
    """
    if credential.provider == 'cloudflare':
        return CloudflareClient(credential)
    raise ValueError(f'No shared DNS client for provider: {credential.provider}')
