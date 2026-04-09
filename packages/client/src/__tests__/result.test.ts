import { describe, it } from "node:test";
import assert from "node:assert";
import { rpcUnwrap, mapError, isAppError, isTransportError } from "../result";
import { TeleportError, TransportFailure } from "../errors";
import type { RpcResult } from "../types";

describe("rpcUnwrap", () => {
  it("returns data on success", () => {
    const result: RpcResult<string> = {
      kind: "success",
      ok: true,
      data: "hello",
    };
    assert.strictEqual(rpcUnwrap(result), "hello");
  });

  it("throws TeleportError on app error", () => {
    const result: RpcResult<string> = {
      kind: "error",
      ok: false,
      error: { type: "NotFound" },
    };
    assert.throws(() => rpcUnwrap(result), TeleportError);
  });

  it("throws TransportFailure on transport error", () => {
    const result: RpcResult<string> = {
      kind: "transport",
      ok: false,
      transport: { type: "Timeout", message: "timed out" },
    };
    assert.throws(() => rpcUnwrap(result), TransportFailure);
  });
});

describe("mapError", () => {
  it("returns data on success", () => {
    const result: RpcResult<string> = {
      kind: "success",
      ok: true,
      data: "hello",
    };
    assert.strictEqual(
      mapError(result, () => "fallback"),
      "hello",
    );
  });

  it("calls handler on app error", () => {
    const result: RpcResult<string> = {
      kind: "error",
      ok: false,
      error: { type: "NotFound" },
    };
    assert.strictEqual(
      mapError(result, (e) => e.type),
      "NotFound",
    );
  });

  it("throws TransportFailure on transport error", () => {
    const result: RpcResult<string> = {
      kind: "transport",
      ok: false,
      transport: { type: "NetworkError", message: "offline" },
    };
    assert.throws(
      () => mapError(result, () => "ignored"),
      TransportFailure,
    );
  });
});

describe("isAppError", () => {
  it("returns true for app errors", () => {
    const result: RpcResult<string> = {
      kind: "error",
      ok: false,
      error: { type: "NotFound" },
    };
    assert.strictEqual(isAppError(result), true);
  });

  it("returns false for success", () => {
    const result: RpcResult<string> = {
      kind: "success",
      ok: true,
      data: "hello",
    };
    assert.strictEqual(isAppError(result), false);
  });
});

describe("isTransportError", () => {
  it("returns true for transport errors", () => {
    const result: RpcResult<string> = {
      kind: "transport",
      ok: false,
      transport: { type: "Timeout", message: "timed out" },
    };
    assert.strictEqual(isTransportError(result), true);
  });

  it("returns false for success", () => {
    const result: RpcResult<string> = {
      kind: "success",
      ok: true,
      data: "hello",
    };
    assert.strictEqual(isTransportError(result), false);
  });
});
