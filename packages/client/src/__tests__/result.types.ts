import {
  TeleportError,
  isAppError,
  isTransportError,
  mapError,
  rpcUnwrap,
} from "../index";
import type { AppError, RpcResult, TransportError } from "../index";

type Equal<A, B> = (
  (<T>() => T extends A ? 1 : 2) extends
  (<T>() => T extends B ? 1 : 2) ? true : false
);
type Expect<T extends true> = T;

type User = {
  id: string;
  name: string;
};

type ValidationDetail = {
  field: "email" | "password";
  reason: string;
};

declare const result: RpcResult<User, ValidationDetail>;

if (isAppError(result)) {
  type _AppErrorNarrowing = Expect<
    Equal<typeof result.error, AppError<ValidationDetail>>
  >;

  if (result.error.type === "Detail") {
    const field: "email" | "password" = result.error.detail.field;
    const reason: string = result.error.detail.reason;
    void field;
    void reason;
  }
} else if (isTransportError(result)) {
  type _TransportErrorNarrowing = Expect<
    Equal<typeof result.transport, TransportError>
  >;
} else {
  const name: string = result.data.name;
  void name;
}

const unwrapped = rpcUnwrap(result);
type _RpcUnwrapResult = Expect<Equal<typeof unwrapped, User>>;

const mapped = mapError<
  User,
  ValidationDetail,
  AppError<ValidationDetail>["type"] | ValidationDetail["field"]
>(
  result,
  (error) => (error.type === "Detail" ? error.detail.field : error.type),
);
type _MapErrorResult = Expect<
  Equal<
    typeof mapped,
    User | AppError<ValidationDetail>["type"] | ValidationDetail["field"]
  >
>;

const typedTeleportError = new TeleportError<ValidationDetail>({
  type: "Detail",
  detail: {
    field: "email",
    reason: "already taken",
  },
});

type _TeleportErrorDetail = Expect<
  Equal<typeof typedTeleportError.detail, ValidationDetail | undefined>
>;

if (typedTeleportError.is("Detail") && typedTeleportError.detail) {
  type _TeleportErrorDetailBranch = Expect<
    Equal<
      typeof typedTeleportError.appError,
      Extract<AppError<ValidationDetail>, { type: "Detail" }>
    >
  >;
  const field: "email" | "password" = typedTeleportError.detail.field;
  void field;
}

const notFoundTeleportError = new TeleportError<ValidationDetail>({
  type: "NotFound",
});

if (notFoundTeleportError.is("NotFound")) {
  type _TeleportErrorNotFoundBranch = Expect<
    Equal<
      typeof notFoundTeleportError.appError,
      Extract<AppError<ValidationDetail>, { type: "NotFound" }>
    >
  >;
  type _TeleportErrorNotFoundDetail = Expect<
    Equal<typeof notFoundTeleportError.detail, undefined>
  >;
  const detail: undefined = notFoundTeleportError.detail;
  void detail;
}

// @ts-expect-error invalid Teleport app-error variant
typedTeleportError.is("NotFoud");

export {};
