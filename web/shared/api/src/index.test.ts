import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The client reads the page's origin when the module loads, so each test
// stubs the globals first and then imports a fresh copy.
async function load() {
  const { echo } = await import("./index");
  return echo;
}

describe("echo", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("location", new URL("http://localhost"));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns the message the server echoes", async () => {
    const fetch = vi.fn().mockResolvedValue(Response.json({ message: "hi" }));
    vi.stubGlobal("fetch", fetch);
    const echo = await load();

    await expect(echo("hi")).resolves.toBe("hi");

    const request: Request = fetch.mock.calls[0]?.[0];
    expect(request.url).toBe("http://localhost/api/echo");
    await expect(request.json()).resolves.toEqual({ message: "hi" });
  });

  it("throws on an error status", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response("nope", { status: 502 })),
    );
    const echo = await load();

    await expect(echo("hi")).rejects.toThrow(/502/);
  });
});
