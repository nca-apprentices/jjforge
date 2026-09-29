import { afterEach, describe, expect, it, vi } from "vitest";
import { echo } from "./api";

describe("echo", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns the message the server echoes", async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(Response.json({ message: "hello" }));
    vi.stubGlobal("fetch", fetch);

    await expect(echo("hello")).resolves.toBe("hello");
    expect(fetch).toHaveBeenCalledWith(
      "/api/echo",
      expect.objectContaining({ body: '{"message":"hello"}' }),
    );
  });

  it("throws on an error status", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response("nope", { status: 502 })),
    );

    await expect(echo("hello")).rejects.toThrow(/502/);
  });
});
