import type { ReactNode } from "react";

/**
 * The main region of a page, under its title.
 * @public
 */
export function Page({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <main className="mx-auto flex max-w-2xl flex-col gap-6 p-6">
      <h1 className="font-semibold text-2xl">{title}</h1>
      {children}
    </main>
  );
}
