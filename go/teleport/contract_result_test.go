package teleport

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"
)

func TestContractHelpersAndErrorSerialization(t *testing.T) {
	t.Run("bundle sorting and method strings", func(t *testing.T) {
		bundle := ContractBundle{
			Version: ContractVersion,
			Procedures: []ProcedureContract{
				{Name: "zeta.list"},
				{Name: "alpha.get"},
			},
			Types: []NamedTypeContract{
				{Name: "Zulu"},
				{Name: "Alpha"},
			},
		}

		bundle.Sort()
		if bundle.Procedures[0].Name != "alpha.get" || bundle.Types[0].Name != "Alpha" {
			t.Fatalf("bundle not sorted %#v", bundle)
		}

		if MethodGet.String() != http.MethodGet {
			t.Fatalf("unexpected GET method string %q", MethodGet.String())
		}
		if MethodPost.String() != http.MethodPost {
			t.Fatalf("unexpected POST method string %q", MethodPost.String())
		}
		if HTTPMethod("Invalid").String() != "" {
			t.Fatalf("unexpected default method string %q", HTTPMethod("Invalid").String())
		}
	})

	t.Run("type constructors marshal contract-compatible shapes", func(t *testing.T) {
		eventFields := NamedFields(
			Field("items", ListExpr(NamedExpr("User", GenericExpr("T")))),
			OptionalField("cursor", NullableExpr(PrimitiveExpr(PrimitiveStr))),
		)
		enum := NamedTypeContract{
			Name:     "Event",
			Generics: []string{"T"},
			Kind: EnumKind(
				VariantContract{Name: "Started", Fields: UnitFields()},
				VariantContract{
					Name: "Moved",
					Fields: UnnamedFields(UnnamedFieldContract{
						Ty: typeExprPtr(MapExpr(PrimitiveExpr(PrimitiveStr), OpaqueExpr("json.RawMessage"))),
					}),
				},
			),
		}
		structType := StructType("Envelope", eventFields)

		body, err := json.Marshal([]NamedTypeContract{structType, enum})
		if err != nil {
			t.Fatalf("marshal contracts: %v", err)
		}
		jsonBody := string(body)
		for _, want := range []string{
			`"List":{"Named":{"generics":[{"Generic":"T"}],"name":"User"}}`,
			`"cursor","docs":"","optional":true,"ty":{"Nullable":{"Primitive":"str"}}`,
			`"Started","docs":"","fields":"Unit"`,
			`"Moved","docs":"","fields":{"Unnamed":[{"docs":"","ty":{"Map":{"key":{"Primitive":"str"},"value":{"Opaque":"json.RawMessage"}}}}]}`,
		} {
			if !strings.Contains(jsonBody, want) {
				t.Fatalf("expected %q in %s", want, jsonBody)
			}
		}
	})

	t.Run("error constructors, status codes, and result helpers", func(t *testing.T) {
		detail := map[string]string{"field": "title"}
		cases := []struct {
			err        *AppError
			wantStatus int
			wantBody   string
		}{
			{UnauthorizedError(), http.StatusUnauthorized, `"type":"Unauthorized"`},
			{ForbiddenError(), http.StatusForbidden, `"type":"Forbidden"`},
			{NotFoundError(), http.StatusNotFound, `"type":"NotFound"`},
			{BadRequestError("bad input"), http.StatusBadRequest, `"message":"bad input"`},
			{InternalError("hidden"), http.StatusInternalServerError, `"message":"hidden"`},
			{RateLimitedError(), http.StatusTooManyRequests, `"type":"RateLimited"`},
			{DetailError(detail), http.StatusUnprocessableEntity, `"detail":{"field":"title"}`},
			{&AppError{Type: AppErrorType("Other")}, http.StatusInternalServerError, `"type":"Other"`},
		}

		for _, tc := range cases {
			if got := tc.err.StatusCode(); got != tc.wantStatus {
				t.Fatalf("unexpected status for %#v: %d", tc.err, got)
			}
			body, err := json.Marshal(tc.err)
			if err != nil {
				t.Fatalf("marshal error %v: %v", tc.err, err)
			}
			if !strings.Contains(string(body), tc.wantBody) {
				t.Fatalf("expected %q in %s", tc.wantBody, string(body))
			}
		}

		if (*AppError)(nil).StatusCode() != http.StatusInternalServerError {
			t.Fatal("expected nil app error to default to 500")
		}

		ok := Ok("done")
		if ok.Value != "done" || ok.Error != nil {
			t.Fatalf("unexpected ok result %#v", ok)
		}

		failed := Fail[string](BadRequestError("bad input"))
		if failed.Error == nil || failed.Error.Type != AppErrorBadRequest {
			t.Fatalf("unexpected fail result %#v", failed)
		}
	})
}

func typeExprPtr(expr TypeExpr) *TypeExpr {
	return &expr
}
