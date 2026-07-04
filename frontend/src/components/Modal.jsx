import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog';
import { cn } from '@/lib/utils';

// Generic modal built on the Radix Dialog primitive. SCSS owns all geometry
// (styles/components/_ui.scss → .sk-modal*). `size` picks a max-width on
// >=sm screens; mobile is always near full-width.
export default function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  className = '',
  size = 'md',
}) {
  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className={cn('sk-modal', `sk-modal--${size}`, className)}>
        {title && (
          <div className="sk-modal__header">
            <DialogTitle>{title}</DialogTitle>
          </div>
        )}

        <div className="sk-modal__body">{children}</div>

        {footer && <div className="sk-modal__footer">{footer}</div>}
      </DialogContent>
    </Dialog>
  );
}
