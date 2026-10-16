import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** The class merger the coss ui components import: `clsx` conditionals, then conflicts resolved by `tailwind-merge`. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
