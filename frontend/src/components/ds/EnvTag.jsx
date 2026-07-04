import { cn } from '@/lib/utils';

const ENV_MOD = { PROD: 'prod', PRODUCTION: 'prod', DEV: 'dev', DEVELOPMENT: 'dev', STAGING: 'staging' };

// Environment / category tag, e.g. <EnvTag env="PROD" />. SERVICE / unknown
// values render as the neutral tag style.
export function EnvTag({ env, className, children, ...props }) {
    const label = children ?? env;
    const mod = ENV_MOD[String(env ?? children ?? '').toUpperCase()];
    return (
        <span className={cn('sk-tag', mod && `sk-tag--${mod}`, className)} {...props}>
            {label}
        </span>
    );
}

export default EnvTag;
