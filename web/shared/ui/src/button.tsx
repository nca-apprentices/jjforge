import type { ReactNode } from "react";

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
      className="inline-flex h-9 items-center justify-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 font-medium text-primary-foreground text-sm shadow-xs outline-none transition-all hover:bg-primary/90 focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50"
    >
      {children}
    </button>
  );
}
