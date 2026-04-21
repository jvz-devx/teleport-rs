import assert from "node:assert";
import { describe, it } from "node:test";
import { createClient } from "../client";
import { installFetchMock, jsonResponse } from "./helpers";

describe("createClient GET requests", () => {
  it("serializes nested query input, skips nulls, and forwards async config", async () => {
    const fetchMock = installFetchMock(() =>
      jsonResponse({ users: ["ada", "linus"] }),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
        credentials: "same-origin",
        timeout: 2_000,
        headers: async () => ({
          Authorization: "Bearer token",
          "X-Trace-Id": "trace-123",
        }),
      });

      const result = await client.rpc<{ users: string[] }, never>(
        "GET",
        "/users",
        {
          filters: {
            role: "admin",
            tags: ["alpha", "beta"],
            omitted: null,
          },
          page: 2,
        },
      );

      assert.deepStrictEqual(result, {
        kind: "success",
        ok: true,
        data: { users: ["ada", "linus"] },
      });
      assert.strictEqual(fetchMock.calls.length, 1);

      const [{ input, init }] = fetchMock.calls;
      const url = new URL(String(input));

      assert.strictEqual(url.origin, "https://example.com");
      assert.strictEqual(url.pathname, "/api/users");
      assert.strictEqual(url.searchParams.get("filters[role]"), "admin");
      assert.strictEqual(url.searchParams.get("filters[tags][0]"), "alpha");
      assert.strictEqual(url.searchParams.get("filters[tags][1]"), "beta");
      assert.strictEqual(url.searchParams.has("filters[omitted]"), false);
      assert.strictEqual(url.searchParams.get("page"), "2");
      assert.strictEqual(init?.method, "GET");
      assert.strictEqual(init?.credentials, "same-origin");
      assert.strictEqual(init?.body, undefined);
      assert.deepStrictEqual(init?.headers, {
        Authorization: "Bearer token",
        "X-Trace-Id": "trace-123",
      });
    } finally {
      fetchMock.restore();
    }
  });

  it("uses injected fetch instead of global fetch", async () => {
    const originalFetch = globalThis.fetch;
    const calls: Array<{ input: RequestInfo | URL; init?: RequestInit }> = [];
    const injectedFetch: typeof fetch = async (
      input: RequestInfo | URL,
      init?: RequestInit,
    ): Promise<Response> => {
      calls.push({ input, init });
      return jsonResponse({ ok: true });
    };

    globalThis.fetch = (() => {
      throw new Error("global fetch should not be called");
    }) as typeof fetch;

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
        fetch: injectedFetch,
      });

      const result = await client.rpc<{ ok: true }, never>(
        "GET",
        "/status",
        undefined,
      );

      assert.deepStrictEqual(result, {
        kind: "success",
        ok: true,
        data: { ok: true },
      });
      assert.strictEqual(calls.length, 1);
      assert.strictEqual(String(calls[0]?.input), "https://example.com/api/status");
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
