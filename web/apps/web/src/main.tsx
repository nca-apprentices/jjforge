import { EchoPage } from "@jjforge/feature-echo";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

// The app owns routing: a feature exposes a page, and a route here mounts it.
const rootRoute = createRootRoute();
const echoRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: EchoPage,
});
const router = createRouter({ routeTree: rootRoute.addChildren([echoRoute]) });
const queryClient = new QueryClient();

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

const container = document.getElementById("root");
if (!container) {
  throw new Error("missing #root");
}

createRoot(container).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
);
