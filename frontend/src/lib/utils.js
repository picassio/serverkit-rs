import { clsx } from 'clsx';

// Join class names. With Tailwind removed there are no conflicting utility
// classes to dedupe, so plain clsx (truthy-aware join) is sufficient — we no
// longer depend on tailwind-merge.
export function cn(...inputs) {
  return clsx(inputs);
}
