import type { ReactNode } from "react";

/**
 * The result of an action, announced to assistive technology.
 * @public
 */
export function Output({ children }: { children: ReactNode }) {
  return <output className="text-muted-foreground text-sm">{children}</output>;
}
