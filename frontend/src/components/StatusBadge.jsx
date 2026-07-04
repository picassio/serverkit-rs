import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

const STATUS_VARIANT = {
  online: 'success',   running: 'success',  healthy: 'success',
  active: 'success',   connected: 'success',
  offline: 'destructive', stopped: 'destructive', error: 'destructive',
  failed: 'destructive',  disconnected: 'destructive',
  warning: 'warning',  degraded: 'warning',
  pending: 'info',     building: 'info',    deploying: 'info',
  paused: 'secondary', unknown: 'secondary',
};

const DOT_COLOR = {
  success: 'bg-green-400',
  destructive: 'bg-red-400',
  warning: 'bg-yellow-400',
  info: 'bg-blue-400',
  secondary: 'bg-muted-foreground/60',
};

const STATUS_LABEL = {
  online: 'Online',   running: 'Running',  healthy: 'Healthy',
  active: 'Active',   connected: 'Connected',
  offline: 'Offline', stopped: 'Stopped',  error: 'Error',
  failed: 'Failed',   disconnected: 'Disconnected',
  warning: 'Warning', degraded: 'Degraded',
  pending: 'Pending', building: 'Building', deploying: 'Deploying',
  paused: 'Paused',   unknown: 'Unknown',
};

export default function StatusBadge({ status, label, className = '' }) {
  const key = status?.toLowerCase();
  const variant = STATUS_VARIANT[key] || 'secondary';
  const dotColor = DOT_COLOR[variant] || DOT_COLOR.secondary;
  const displayLabel = label || STATUS_LABEL[key] || status;

  return (
    <Badge variant={variant} className={cn('status-badge-token', className)}>
      <span className={cn('size-1.5 rounded-full shrink-0', dotColor)} />
      {displayLabel}
    </Badge>
  );
}
