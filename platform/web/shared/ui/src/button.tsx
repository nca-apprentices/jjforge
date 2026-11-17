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
        "text-sm font-medium whitespace-nowrap",
        "bg-primary text-primary-foreground rounded-md shadow-xs",
        "hover:bg-primary/90 transition-all outline-none",
        "focus-visible:ring-ring/50 focus-visible:ring-[3px]",
        "disabled:pointer-events-none disabled:opacity-50",
      )}
    >
      {children}
    </button>
  );
}
