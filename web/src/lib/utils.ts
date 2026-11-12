import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

/** Fusionne des classes Tailwind (convention shadcn/ui) : la dernière l'emporte. */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
