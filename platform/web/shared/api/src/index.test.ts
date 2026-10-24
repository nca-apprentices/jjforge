import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The client reads the page's origin when the module loads, so each test
// stubs the globals first and then imports a fresh copy.
async function load() {
  const { echo } = await import("./index");
  return echo;
}

const trace = "4bf92f3577b34da6a3ce929d0e0e4736";
const traceresponse = `00-${trace}-00f067aa0ba902b7-01`;

describe("echo", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("location", new URL("http://localhost"));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns the message the server echoes and its trace", async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(
        Response.json({ message: "hi" }, { headers: { traceresponse } }),
      );
    vi.stubGlobal("fetch", fetch);
    const echo = await load();

    await expect(echo("hi")).resolves.toEqual({
      message: "hi",
      traceId: trace,
    });

    const request: Request = fetch.mock.calls[0]?.[0];
    expect(request.url).toBe("http://localhost/api/v1/echo");
    await expect(request.json()).resolves.toEqual({ message: "hi" });
  });

  it("throws on an error status, naming the trace", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue(
          new Response("nope", { status: 502, headers: { traceresponse } }),
        ),
    );
    const echo = await load();

    await expect(echo("hi")).rejects.toThrow(`trace ${trace}`);
  });
});
