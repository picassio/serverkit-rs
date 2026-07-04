import * as React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cn } from '@/lib/utils';

// Map shadcn-style `variant` prop to our SCSS .btn-* classes.
// SCSS owns all geometry, color and hover state — see styles/components/_buttons.scss
const VARIANT_CLASS = {
  default:     'btn-primary',
  primary:     'btn-primary',
  destructive: 'btn-danger',
  danger:      'btn-danger',
  outline:     'btn-secondary',
  secondary:   'btn-soft',
  ghost:       'btn-ghost',
  link:        'btn-link',
};

const SIZE_CLASS = {
  default: '',
  md:      '',
  sm:      'btn-sm',
  lg:      'btn-lg',
  icon:    'btn-icon',
};

function buttonVariants({ variant = 'default', size = 'default', className } = {}) {
  return cn(
    'btn',
    VARIANT_CLASS[variant] ?? VARIANT_CLASS.default,
    SIZE_CLASS[size] ?? '',
    className,
  );
}

function Button({ className, variant, size, asChild = false, ...props }) {
  const Comp = asChild ? Slot : 'button';
  return (
    <Comp
      data-slot="button"
      className={buttonVariants({ variant, size, className })}
      {...props}
    />
  );
}

export { Button, buttonVariants };

