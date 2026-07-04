import { cn } from '@/lib/utils';

function Skeleton({ className, ...props }) {
  return <div className={cn('ui-skeleton', className)} {...props} />;
}

export { Skeleton };
