import tailwindcss from "@tailwindcss/vite";

/**
 * The Vite plugins @jjforge/ui needs, so an app's build never names Tailwind.
 * @public
 */
export function ui() {
  return tailwindcss();
}
