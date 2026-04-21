import assert from "node:assert";
import { describe, it } from "node:test";
import { createClient } from "../client";
import { installFetchMock, jsonResponse } from "./helpers";
import type { AppError, TransportError } from "../types";

describe("createClient error handling", () => {
  it("returns an app error and notifies onError for valid 422 Detail responses", async () => {
    const events: Array<
      | { type: "app"; error: AppError<unknown> }
      | { type: "transport"; error: TransportError }
    > = [];
    const fetchMock = installFetchMock(() =>
      jsonResponse(
        {
          type: "Detail",
          detail: { field: "email", reason: "already taken" },
        },
        { status: 422 },
      ),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
        onError: (event) => events.push(event),
      });

      const result = await client.rpc<
        never,
        { field: string; reason: string }
      >("POST", "/users", { email: "ada@example.com" });

      assert.deepStrictEqual(result, {
        kind: "error",
        ok: false,
        error: {
          type: "Detail",
          detail: { field: "email", reason: "already taken" },
        },
      });
      assert.deepStrictEqual(events, [
        {
          type: "app",
          error: {
            type: "Detail",
            detail: { field: "email", reason: "already taken" },
          },
        },
      ]);
    } finally {
      fetchMock.restore();
    }
  });

  it("returns an app error for valid 400 BadRequest responses", async () => {
    const fetchMock = installFetchMock(() =>
      jsonResponse(
        {
          type: "BadRequest",
          message: "email is required",
        },
        { status: 400 },
      ),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
      });

      const result = await client.rpc<never, never>("POST", "/users", {});

      assert.deepStrictEqual(result, {
        kind: "error",
        ok: false,
        error: {
          type: "BadRequest",
          message: "email is required",
        },
      });
    } finally {
      fetchMock.restore();
    }
  });

  it("returns an app error for valid 500 Internal responses", async () => {
    const fetchMock = installFetchMock(() =>
      jsonResponse(
        {
          type: "Internal",
          message: "trace id 123",
        },
        { status: 500 },
      ),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
      });

      const result = await client.rpc<never, never>("GET", "/users", undefined);

      assert.deepStrictEqual(result, {
        kind: "error",
        ok: false,
        error: {
          type: "Internal",
          message: "trace id 123",
        },
      });
    } finally {
      fetchMock.restore();
    }
  });

  it("returns a ServerError transport failure for non-JSON error responses", async () => {
    const events: Array<
      | { type: "app"; error: AppError<unknown> }
      | { type: "transport"; error: TransportError }
    > = [];
    const fetchMock = installFetchMock(
      () =>
        (({
          ok: false,
          status: 502,
          async json() {
            throw new Error("invalid json");
          },
          async text() {
            return "upstream exploded";
          },
        }) as unknown as Response),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
        onError: (event) => events.push(event),
      });

      const result = await client.rpc<never, never>("GET", "/health", undefined);

      assert.deepStrictEqual(result, {
        kind: "transport",
        ok: false,
        transport: {
          type: "ServerError",
          status: 502,
          body: "upstream exploded",
        },
      });
      assert.deepStrictEqual(events, [
        {
          type: "transport",
          error: {
            type: "ServerError",
            status: 502,
            body: "upstream exploded",
          },
        },
      ]);
    } finally {
      fetchMock.restore();
    }
  });

  it("treats valid app-error JSON with the wrong status as a transport failure", async () => {
    const events: Array<
      | { type: "app"; error: AppError<unknown> }
      | { type: "transport"; error: TransportError }
    > = [];
    const body = JSON.stringify({
      type: "BadRequest",
      message: "email is required",
    });
    const fetchMock = installFetchMock(
      () =>
        new Response(body, {
          status: 422,
          headers: { "Content-Type": "application/json" },
        }),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
        onError: (event) => events.push(event),
      });

      const result = await client.rpc<never, never>("POST", "/users", {});

      assert.deepStrictEqual(result, {
        kind: "transport",
        ok: false,
        transport: {
          type: "ServerError",
          status: 422,
          body,
        },
      });
      assert.deepStrictEqual(events, [
        {
          type: "transport",
          error: {
            type: "ServerError",
            status: 422,
            body,
          },
        },
      ]);
    } finally {
      fetchMock.restore();
    }
  });

  it("treats parsed JSON that is not an AppError variant as a transport failure", async () => {
    const events: Array<
      | { type: "app"; error: AppError<unknown> }
      | { type: "transport"; error: TransportError }
    > = [];
    const body = JSON.stringify({
      error: "proxy failed",
    });
    const fetchMock = installFetchMock(
      () =>
        new Response(body, {
          status: 502,
          headers: { "Content-Type": "application/json" },
        }),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
        onError: (event) => events.push(event),
      });

      const result = await client.rpc<never, never>("GET", "/health", undefined);

      assert.deepStrictEqual(result, {
        kind: "transport",
        ok: false,
        transport: {
          type: "ServerError",
          status: 502,
          body,
        },
      });
      assert.deepStrictEqual(events, [
        {
          type: "transport",
          error: {
            type: "ServerError",
            status: 502,
            body,
          },
        },
      ]);
    } finally {
      fetchMock.restore();
    }
  });

  it("maps thrown fetch failures to NetworkError transport results", async () => {
    const events: Array<
      | { type: "app"; error: AppError<unknown> }
      | { type: "transport"; error: TransportError }
    > = [];
    const fetchMock = installFetchMock(() => {
      throw new Error("socket closed");
    });

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
        onError: (event) => events.push(event),
      });

      const result = await client.rpc<never, never>("GET", "/users", undefined);

      assert.deepStrictEqual(result, {
        kind: "transport",
        ok: false,
        transport: {
          type: "NetworkError",
          message: "socket closed",
        },
      });
      assert.deepStrictEqual(events, [
        {
          type: "transport",
          error: {
            type: "NetworkError",
            message: "socket closed",
          },
        },
      ]);
    } finally {
      fetchMock.restore();
    }
  });

  it("maps aborted requests to Timeout transport results", async () => {
    const events: Array<
      | { type: "app"; error: AppError<unknown> }
      | { type: "transport"; error: TransportError }
    > = [];
    const fetchMock = installFetchMock(
      (_input, init) =>
        new Promise<Response>((_resolve, reject) => {
          init?.signal?.addEventListener("abort", () => {
            reject(new DOMException("Aborted", "AbortError"));
          });
        }),
    );

    try {
      const client = createClient({
        baseUrl: "https://example.com/api",
        timeout: 5,
        onError: (event) => events.push(event),
      });

      const result = await client.rpc<never, never>("GET", "/slow", undefined);

      assert.deepStrictEqual(result, {
        kind: "transport",
        ok: false,
        transport: {
          type: "Timeout",
          message: "Request timed out after 5ms",
        },
      });
      assert.deepStrictEqual(events, [
        {
          type: "transport",
          error: {
            type: "Timeout",
            message: "Request timed out after 5ms",
          },
        },
      ]);
    } finally {
      fetchMock.restore();
    }
  });
});
