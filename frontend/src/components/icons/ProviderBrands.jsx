// Brand-authentic icons for external connection providers — the single source of
// truth for how an integration (GitHub, Cloudflare, DigitalOcean, GoDaddy, …) is
// presented across the Connections hub. Mirrors components/icons/DatabaseBrands.jsx
// and components/git/GitProviders.jsx: we wrap Simple Icons (via react-icons) so
// each provider is instantly recognizable instead of sharing one generic glyph.
//
// Simple Icons render with `fill="currentColor"`, so the surrounding SCSS controls
// the color with no inline styles.
import {
    SiGithub, SiGitlab, SiBitbucket, SiCloudflare, SiDigitalocean,
    SiHetzner, SiVultr, SiAkamai, SiGodaddy, SiNamecheap, SiBackblaze, SiDocker,
} from 'react-icons/si';
import { FaAws } from 'react-icons/fa';
import { Mail, HardDrive, Plug } from 'lucide-react';

// provider id (matches providerCatalog) -> brand icon component. Several ids map
// to the same brand wearing two hats (e.g. DigitalOcean as both a cloud host and
// a DNS provider; GoDaddy as both DNS and registrar).
const PROVIDER_ICONS = {
    github: SiGithub,
    gitlab: SiGitlab,
    bitbucket: SiBitbucket,
    cloudflare: SiCloudflare,
    route53: FaAws,
    digitalocean: SiDigitalocean,
    digitalocean_dns: SiDigitalocean,
    hetzner: SiHetzner,
    vultr: SiVultr,
    linode: SiAkamai,        // Linode is now Akamai
    godaddy: SiGodaddy,
    godaddy_dns: SiGodaddy,
    namecheap: SiNamecheap,
    container_registry: SiDocker,
    smtp: Mail,
    s3: HardDrive,
    b2: SiBackblaze,
};

// Renders the brand icon for a provider, falling back to the generic lucide Plug
// glyph for anything we don't have a brand for.
export function ProviderBrandIcon({ provider, size = 22, className }) {
    const Cmp = PROVIDER_ICONS[provider] || Plug;
    return <Cmp size={size} className={className} aria-hidden="true" />;
}

export default ProviderBrandIcon;
