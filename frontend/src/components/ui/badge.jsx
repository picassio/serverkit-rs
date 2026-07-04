import * as React from 'react';
import { cn } from '@/lib/utils';

// Styling lives in styles/components/_badges.scss via the
// `[data-slot="badge"][data-variant="..."]` selectors. This component is
// just a thin wrapper that sets those data attributes.
function Badge({ className, variant, ...props }) {
  return (
    <span
      data-slot="badge"
      data-variant={variant || 'default'}
      className={cn(className)}
      {...props}
    />
  );
}

export { Badge };
