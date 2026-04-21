import assert from "node:assert";
import { describe, it } from "node:test";
import { createClient } from "../client";
import { installFetchMock, jsonResponse, textResponse } from "./helpers";

describe("createClient POST requests", () => {
  it("sends JSON bodies and preserves custom headers", async () => {
    const fetchMock = installFetchMock(() =>
      jsonResponse({ id: "user-1", name: "Ada" }),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
        headers: () => ({
          Authorization: "Bearer token",
          "X-Client-Version": "test-suite",
        }),
      });

      const result = await client.rpc<{ id: string; name: string }, never>(
        "POST",
        "/users",
        {
          name: "Ada",
          nested: { enabled: true },
        },
      );

      assert.deepStrictEqual(result, {
        kind: "success",
        ok: true,
        data: { id: "user-1", name: "Ada" },
      });
      assert.strictEqual(fetchMock.calls.length, 1);

      const [{ input, init }] = fetchMock.calls;
      assert.strictEqual(String(input), "https://example.com/api/users");
      assert.strictEqual(init?.method, "POST");
      assert.deepStrictEqual(init?.headers, {
        Authorization: "Bearer token",
        "X-Client-Version": "test-suite",
        "Content-Type": "application/json",
      });
      assert.strictEqual(
        init?.body,
        JSON.stringify({ name: "Ada", nested: { enabled: true } }),
      );
    } finally {
      fetchMock.restore();
    }
  });

  it("treats empty successful responses as undefined data", async () => {
    const fetchMock = installFetchMock(() =>
      textResponse("", { status: 200 }),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
      });

      const result = await client.rpc<undefined, never>(
        "POST",
        "/sessions/refresh",
        undefined,
      );

      assert.deepStrictEqual(result, {
        kind: "success",
        ok: true,
        data: undefined,
      });
    } finally {
      fetchMock.restore();
    }
  });
});
