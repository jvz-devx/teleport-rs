package teleport_test

import (
	"encoding/json"
	"reflect"
	"strings"
	"testing"

	"github.com/jvz-devx/teleport-rs/go/teleport"
)

type getUserInput struct {
	ID string `json:"id"`
}

type user struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

type userID string

type nestedProfile struct {
	ID userID `json:"id"`
}

type userEnvelope struct {
	Profile nestedProfile `json:"profile"`
}

func TestExportContract(t *testing.T) {
	procedures := []teleport.Procedure{
		teleport.QueryWithErrorFor[getUserInput, user, map[string]bool]("users", "getUser").
			Doc("Fetch a user.").
			Handle(func(ctx teleport.RequestContext, input getUserInput) teleport.Result[user] {
				return teleport.Ok(user{ID: input.ID, Name: "Ada"})
			}),
	}

	bundle := teleport.NewContractBundle()
	bundle.Procedures = append(bundle.Procedures, procedures[0].Contract)
	bundle.Types = append(bundle.Types, teleport.ExportNamedTypes(reflect.TypeOf(getUserInput{}), reflect.TypeOf(user{}))...)
	bundle.Sort()

	if bundle.Version != teleport.ContractVersion {
		t.Fatalf("unexpected contract version: %s", bundle.Version)
	}
	if len(bundle.Procedures) != 1 {
		t.Fatalf("expected one procedure, got %d", len(bundle.Procedures))
	}
	if bundle.Procedures[0].Path != "/rpc/users.getUser" {
		t.Fatalf("unexpected procedure path: %s", bundle.Procedures[0].Path)
	}
	if bundle.Procedures[0].Doc != "Fetch a user." {
		t.Fatalf("unexpected procedure doc: %q", bundle.Procedures[0].Doc)
	}
	body, err := json.Marshal(bundle)
	if err != nil {
		t.Fatalf("marshal contract: %v", err)
	}
	if len(body) == 0 {
		t.Fatal("expected non-empty json")
	}
	if !strings.Contains(string(body), `"error_type":{"Map":{"key":{"Primitive":"str"},"value":{"Primitive":"bool"}}}`) {
		t.Fatalf("unexpected error_type in %s", string(body))
	}
}

func TestProcedureBuilderAuthAndKinds(t *testing.T) {
	command := teleport.CommandFor[teleport.Unit, user]("posts", "createPost").
		RequireAuth().
		Doc("Create a post.").
		Handle(func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[user] {
			return teleport.Ok(user{ID: "1", Name: "Ada"})
		})

	if command.Contract.ProcedureKind != teleport.ProcedureCommand {
		t.Fatalf("unexpected kind: %s", command.Contract.ProcedureKind)
	}
	if command.Contract.AuthMode != teleport.AuthRequired {
		t.Fatalf("unexpected auth mode: %s", command.Contract.AuthMode)
	}
	if command.Contract.Doc != "Create a post." {
		t.Fatalf("unexpected doc: %q", command.Contract.Doc)
	}
	if command.Contract.InputEncoding != teleport.InputNone {
		t.Fatalf("unexpected input encoding: %s", command.Contract.InputEncoding)
	}

	form := teleport.FormFor[getUserInput, user]("users", "updateUser").
		OptionalAuth().
		Handle(func(ctx teleport.RequestContext, input getUserInput) teleport.Result[user] {
			return teleport.Ok(user{ID: input.ID, Name: "Ada"})
		})

	if form.Contract.ProcedureKind != teleport.ProcedureForm {
		t.Fatalf("unexpected kind: %s", form.Contract.ProcedureKind)
	}
	if form.Contract.AuthMode != teleport.AuthOptional {
		t.Fatalf("unexpected auth mode: %s", form.Contract.AuthMode)
	}
	if form.Contract.InputEncoding != teleport.InputFormBody {
		t.Fatalf("unexpected input encoding: %s", form.Contract.InputEncoding)
	}
}

func TestExportNamedTypesRecursivelyIncludesAliases(t *testing.T) {
	types := teleport.ExportNamedTypes(reflect.TypeOf(userEnvelope{}))

	body, err := json.Marshal(types)
	if err != nil {
		t.Fatalf("marshal named types: %v", err)
	}
	jsonBody := string(body)

	for _, want := range []string{
		`"name":"userEnvelope"`,
		`"name":"nestedProfile"`,
		`"name":"userID"`,
		`"Alias":{"Primitive":"str"}`,
		`"name":"profile"`,
		`"name":"nestedProfile"`,
		`"name":"id"`,
		`"name":"userID"`,
	} {
		if !strings.Contains(jsonBody, want) {
			t.Fatalf("expected %q in %s", want, jsonBody)
		}
	}
}

func TestQueryRejectsPrimitiveInputs(t *testing.T) {
	assertProcedureConstructionPanics(t, "query inputs must be named struct wrappers", func() {
		_ = teleport.Query[string, string]("users", "find", teleport.AuthNone, func(ctx teleport.RequestContext, input string) teleport.Result[string] {
			return teleport.Ok(input)
		})
	})
}

func TestProcedureBuilderRejectsPointerWrapperInputs(t *testing.T) {
	t.Run("query builder rejects pointer input wrappers", func(t *testing.T) {
		assertProcedureConstructionPanics(t, "query inputs must not be pointers", func() {
			_ = teleport.QueryFor[*getUserInput, user]("users", "findUser").
				Handle(func(ctx teleport.RequestContext, input *getUserInput) teleport.Result[user] {
					return teleport.Ok(user{ID: input.ID, Name: "Ada"})
				})
		})
	})

	t.Run("form builder rejects pointer input wrappers", func(t *testing.T) {
		assertProcedureConstructionPanics(t, "form inputs must not be pointers", func() {
			_ = teleport.FormFor[*getUserInput, user]("users", "updateUser").
				Handle(func(ctx teleport.RequestContext, input *getUserInput) teleport.Result[user] {
					return teleport.Ok(user{ID: input.ID, Name: "Ada"})
				})
		})
	})
}

func assertProcedureConstructionPanics(t *testing.T, want string, build func()) {
	t.Helper()

	defer func() {
		recovered := recover()
		if recovered == nil {
			t.Fatal("expected procedure construction panic")
		}

		got := recovered
		text, ok := recovered.(string)
		if !ok {
			t.Fatalf("unexpected panic type %T: %v", got, got)
		}
		if !strings.Contains(text, want) {
			t.Fatalf("unexpected panic: %v", got)
		}
	}()

	build()
}
