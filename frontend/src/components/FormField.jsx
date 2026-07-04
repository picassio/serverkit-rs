import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

/**
 * Standard form field wrapper: label, hint, error message, and input slot.
 *
 *   <FormField label="Email" htmlFor="email" error="Required" hint="Used for notifications">
 *     <Input id="email" ... />
 *   </FormField>
 */
export function FormField({
    label,
    htmlFor,
    children,
    error,
    hint,
    required = false,
    className,
    labelClassName,
}) {
    const inputId = htmlFor;

    return (
        <div className={cn('form-field', className)}>
            {label && (
                <Label htmlFor={inputId} className={cn('form-field__label', labelClassName)}>
                    {label}
                    {required && <span className="form-field__required" aria-hidden="true">*</span>}
                </Label>
            )}
            <div className="form-field__control">
                {children}
            </div>
            {hint && !error && <p className="form-field__hint">{hint}</p>}
            {error && <p className="form-field__error" role="alert">{error}</p>}
        </div>
    );
}

/**
 * Horizontal row of form fields. Use for side-by-side inputs.
 *
 *   <FormRow>
 *     <FormField label="First name"><Input ... /></FormField>
 *     <FormField label="Last name"><Input ... /></FormField>
 *   </FormRow>
 */
export function FormRow({ children, className }) {
    return (
        <div className={cn('form-row', className)}>
            {children}
        </div>
    );
}

export default FormField;
