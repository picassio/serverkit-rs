import { useState, useEffect } from 'react';
import { AlertTriangle, Info, AlertCircle } from 'lucide-react';
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
  AlertDialogAction,
} from '@/components/ui/alert-dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

// Styling lives in styles/components/_ui.scss (.sk-confirm*).
const iconMap = { danger: AlertTriangle, warning: AlertCircle, info: Info };

export function ConfirmDialog({
  isOpen,
  title,
  message,
  details,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  variant = 'danger',
  requireConfirmation,
  confirmationPlaceholder,
  onConfirm,
  onCancel,
}) {
  const [inputValue, setInputValue] = useState('');

  useEffect(() => { if (isOpen) setInputValue(''); }, [isOpen]);

  const Icon = iconMap[variant] || AlertTriangle;
  const isConfirmDisabled = requireConfirmation && inputValue !== requireConfirmation;

  return (
    <AlertDialog open={isOpen} onOpenChange={(v) => !v && onCancel()}>
      <AlertDialogContent className="sk-confirm">
        <AlertDialogHeader>
          <div className="sk-confirm__head">
            <div className={`sk-confirm__icon sk-confirm__icon--${variant}`}>
              <Icon size={24} />
            </div>
            <div className="sk-confirm__body">
              <AlertDialogTitle>{title}</AlertDialogTitle>
              {message && <AlertDialogDescription>{message}</AlertDialogDescription>}
              {details && <p className="sk-confirm__details">{details}</p>}
            </div>
          </div>
          {requireConfirmation && (
            <div className="sk-confirm__confirm-field">
              <Label>
                Type <strong className="text-foreground">{requireConfirmation}</strong> to confirm:
              </Label>
              <Input
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && !isConfirmDisabled && onConfirm()}
                placeholder={confirmationPlaceholder || requireConfirmation}
                autoFocus
              />
            </div>
          )}
        </AlertDialogHeader>
        <AlertDialogFooter className="sk-confirm__footer">
          <AlertDialogCancel onClick={onCancel}>{cancelText}</AlertDialogCancel>
          <AlertDialogAction
            onClick={onConfirm}
            disabled={isConfirmDisabled}
            variant={variant === 'danger' ? 'destructive' : 'primary'}
          >
            {confirmText}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

export default ConfirmDialog;
