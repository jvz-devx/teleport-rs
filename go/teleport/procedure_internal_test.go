package teleport

import (
	"reflect"
	"testing"
)

type builderInput struct {
	ID string `json:"id"`
}

type builderOutput struct {
	Message string `json:"message"`
}

type builderDetail struct {
	Code string `json:"code"`
}

func TestProcedureConvenienceBuildersAndInvoke(t *testing.T) {
	query := QueryWithError[builderInput, builderOutput, builderDetail](
		"users",
		"find",
		AuthOptional,
		func(ctx RequestContext, input builderInput) Result[builderOutput] {
			user, _ := ctx.User.(string)
			return Ok(builderOutput{Message: user + ":" + input.ID})
		},
	)

	if query.Contract.HTTPMethod != MethodGet {
		t.Fatalf("unexpected query method %s", query.Contract.HTTPMethod)
	}
	if query.Contract.InputEncoding != InputQueryString {
		t.Fatalf("unexpected query encoding %s", query.Contract.InputEncoding)
	}
	if query.Contract.AuthMode != AuthOptional {
		t.Fatalf("unexpected query auth %s", query.Contract.AuthMode)
	}

	ok, value, err := query.Invoke(RequestContext{User: "demo-user"}, builderInput{ID: "42"})
	if !ok || err != nil {
		t.Fatalf("expected successful invoke, ok=%v err=%v", ok, err)
	}
	if got := value.(builderOutput); got.Message != "demo-user:42" {
		t.Fatalf("unexpected query output %#v", got)
	}

	command := Command[builderInput, builderOutput](
		"users",
		"create",
		AuthRequired,
		func(ctx RequestContext, input builderInput) Result[builderOutput] {
			return Ok(builderOutput{Message: "created:" + input.ID})
		},
	)
	if command.Contract.HTTPMethod != MethodPost || command.Contract.InputEncoding != InputJSONBody {
		t.Fatalf("unexpected command contract %#v", command.Contract)
	}

	form := FormWithError[builderInput, builderOutput, builderDetail](
		"users",
		"submit",
		AuthNone,
		func(ctx RequestContext, input builderInput) Result[builderOutput] {
			return Fail[builderOutput](DetailError(builderDetail{Code: input.ID}))
		},
	)
	if form.Contract.ProcedureKind != ProcedureForm || form.Contract.InputEncoding != InputFormBody {
		t.Fatalf("unexpected form contract %#v", form.Contract)
	}

	ok, value, err = form.Invoke(RequestContext{}, builderInput{ID: "bad"})
	if ok || value != (builderOutput{}) || err == nil {
		t.Fatalf("expected failed invoke, ok=%v value=%#v err=%v", ok, value, err)
	}
	if err.Type != AppErrorDetail {
		t.Fatalf("unexpected error type %s", err.Type)
	}
}

func TestProcedureContractCopiesAndDynamicFallbacks(t *testing.T) {
	original := QueryFor[builderInput, builderOutput]("users", "copy").
		Doc("before").
		Handle(func(ctx RequestContext, input builderInput) Result[builderOutput] {
			return Ok(builderOutput{Message: input.ID})
		})

	copied := original.WithDoc("after")
	if original.Contract.Doc != "before" {
		t.Fatalf("expected original doc to remain unchanged, got %q", original.Contract.Doc)
	}
	if copied.Contract.Doc != "after" {
		t.Fatalf("expected copied doc, got %q", copied.Contract.Doc)
	}

	dynamic := CommandWithError[any, any, any](
		"dynamic",
		"echo",
		AuthNone,
		func(ctx RequestContext, input any) Result[any] {
			return Ok(input)
		},
	)
	if dynamic.InputType != reflect.TypeOf(Unit{}) {
		t.Fatalf("expected unit input fallback, got %v", dynamic.InputType)
	}
	if dynamic.OutputType != reflect.TypeOf(Unit{}) {
		t.Fatalf("expected unit output fallback, got %v", dynamic.OutputType)
	}
	if dynamic.ErrorType != reflect.TypeOf(Unit{}) {
		t.Fatalf("expected unit error fallback, got %v", dynamic.ErrorType)
	}
	if dynamic.Contract.InputEncoding != InputNone {
		t.Fatalf("expected no input encoding, got %s", dynamic.Contract.InputEncoding)
	}
}

func TestProcedureKindLabelDefaults(t *testing.T) {
	if got := procedureKindLabel(ProcedureCommand); got != "procedure" {
		t.Fatalf("unexpected command label %q", got)
	}
	if got := procedureKindLabel(ProcedureKind("Custom")); got != "procedure" {
		t.Fatalf("unexpected custom label %q", got)
	}
}
