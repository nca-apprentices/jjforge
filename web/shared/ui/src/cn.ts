import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Joins class names, one string per concern, as shadcn/ui's `cn` does. Of two
 * conflicting Tailwind classes, the later one wins.
 */
export function cn(...classes: ClassValue[]) {
  return twMerge(clsx(classes));
}
