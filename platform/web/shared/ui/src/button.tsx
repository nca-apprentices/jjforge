import type { ReactNode } from "react";
import { cn } from "./cn";

/**
 * A primary button, styled after shadcn/ui's default variant.
 * @public
 */
export function Button({
  children,
  type = "button",
}: {
  children: ReactNode;
  type?: "button" | "submit";
}) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex h-9 items-center justify-center gap-2 px-4 py-2",
        "whitespace-nowrap font-medium text-sm",
        "rounded-md bg-primary text-primary-foreground shadow-xs",
        "outline-none transition-all hover:bg-primary/90",
        "focus-visible:ring-[3px] focus-visible:ring-ring/50",
        "disabled:pointer-events-none disabled:opacity-50",
      )}
    >
      {children}
    </button>
  );
}
