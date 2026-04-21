import { createClient } from "../index";
import type { RpcResult } from "../index";

type Equal<A, B> = (
  (<T>() => T extends A ? 1 : 2) extends
  (<T>() => T extends B ? 1 : 2) ? true : false
);
type Expect<T extends true> = T;

type User = {
  id: string;
  name: string;
  role: "admin" | "member";
};

type UpdateUserInput = {
  name: string;
  newsletter: boolean;
};

type ValidationDetail = {
  issues: Array<{
    field: "name" | "newsletter";
    message: string;
  }>;
};

type NamespaceBinding = {
  getUser(id: string): Promise<RpcResult<User, ValidationDetail>>;
  updateUser(id: string, input: UpdateUserInput): Promise<RpcResult<User, ValidationDetail>>;
};

function bindClient(client: Pick<ReturnType<typeof createClient>, "rpc">): {
  users: NamespaceBinding;
} {
  return {
    users: {
      getUser(id) {
        return client.rpc<User, ValidationDetail>("GET", `/users/${id}`, undefined);
      },
      updateUser(id, input) {
        return client.rpc<User, ValidationDetail>("POST", `/users/${id}`, input);
      },
    },
  };
}

function createUsersApi(basePath = "/users") {
  return {
    getUser(client: Pick<ReturnType<typeof createClient>, "rpc">, id: string) {
      return client.rpc<User, ValidationDetail>("GET", `${basePath}/${id}`, undefined);
    },
    updateUser(
      client: Pick<ReturnType<typeof createClient>, "rpc">,
      id: string,
      input: UpdateUserInput,
    ) {
      return client.rpc<User, ValidationDetail>("POST", `${basePath}/${id}`, input);
    },
  };
}

const directClient = createClient({
  baseUrl: "/api",
  credentials: "include",
});

const directResult = directClient.rpc<User, ValidationDetail>(
  "POST",
  "/users/search",
  { query: "ada" },
);

type _DirectClientResult = Expect<
  Equal<Awaited<typeof directResult>, RpcResult<User, ValidationDetail>>
>;

const usersApi = createUsersApi();
const boundApi = bindClient(directClient);
const getUserResult = usersApi.getUser(directClient, "user-1");
const updateUserResult = usersApi.updateUser(directClient, "user-1", {
  name: "Ada",
  newsletter: true,
});
const boundGetUserResult = boundApi.users.getUser("user-1");

type _GeneratedGetResult = Expect<
  Equal<Awaited<typeof getUserResult>, RpcResult<User, ValidationDetail>>
>;
type _GeneratedUpdateResult = Expect<
  Equal<Awaited<typeof updateUserResult>, RpcResult<User, ValidationDetail>>
>;
type _BoundGetResult = Expect<
  Equal<Awaited<typeof boundGetUserResult>, RpcResult<User, ValidationDetail>>
>;

async function consumeGeneratedClient() {
  const result = await usersApi.updateUser(directClient, "user-1", {
    name: "Ada",
    newsletter: false,
  });

  if (result.ok) {
    const role: "admin" | "member" = result.data.role;
    return role;
  }

  if (result.kind === "error" && result.error.type === "Detail") {
    const issueField: "name" | "newsletter" =
      result.error.detail.issues[0]?.field ?? "name";
    return issueField;
  }

  return result.kind;
}

void consumeGeneratedClient;

// @ts-expect-error newsletter is required for generated wrappers
usersApi.updateUser(directClient, "user-1", { name: "Ada" });

export {};
