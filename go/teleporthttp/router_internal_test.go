package teleporthttp

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"reflect"
	"strconv"
	"strings"
	"testing"

	"github.com/jvz-devx/teleport-rs/go/teleport"
)

type authMessage struct {
	User string `json:"user"`
}

type detailPayload struct {
	Field string `json:"field"`
}

type nestedItem struct {
	Name    string `json:"name"`
	Enabled bool   `json:"enabled"`
}

type complexDecodeInput struct {
	Single []string           `json:"single"`
	Names  []string           `json:"names"`
	Counts map[string]uint8   `json:"counts"`
	Score  float32            `json:"score"`
	Limit  uint64             `json:"limit"`
	Item   *nestedItem        `json:"item"`
	Items  []nestedItem       `json:"items"`
	Meta   map[string]float64 `json:"meta"`
}

type nestedRequiredWrapper struct {
	Item nestedItem `json:"item"`
}

func TestRouterContractAuthAndDetailErrors(t *testing.T) {
	router := New().
		AddTypes(
			teleport.NamedTypeContract{Name: "Zulu"},
			teleport.NamedTypeContract{Name: "Alpha"},
		).
		Register(
			teleport.Query[teleport.Unit, string]("demo", "zeta", teleport.AuthOptional, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[string] {
				if ctx.User == nil {
					return teleport.Ok("anonymous")
				}
				return teleport.Ok(ctx.User.(string))
			}),
			teleport.Query[teleport.Unit, string]("demo", "alpha", teleport.AuthRequired, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[string] {
				return teleport.Ok(ctx.User.(string))
			}),
			teleport.Command[teleport.Unit, string]("demo", "detail", teleport.AuthNone, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[string] {
				return teleport.Fail[string](teleport.DetailError(detailPayload{Field: "title"}))
			}),
		)

	contract := router.Contract()
	if contract.Procedures[0].Name != "demo.alpha" || contract.Types[0].Name != "Alpha" {
		t.Fatalf("contract not sorted %#v", contract)
	}

	t.Run("required auth without authenticator returns 401", func(t *testing.T) {
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/rpc/demo.alpha", nil))
		if rec.Code != http.StatusUnauthorized {
			t.Fatalf("unexpected status %d body=%s", rec.Code, rec.Body.String())
		}
	})

	t.Run("optional auth without authenticator still runs handler", func(t *testing.T) {
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/rpc/demo.zeta", nil))
		if rec.Code != http.StatusOK || strings.TrimSpace(rec.Body.String()) != `"anonymous"` {
			t.Fatalf("unexpected optional auth response %d %s", rec.Code, rec.Body.String())
		}
	})

	t.Run("detail errors preserve typed payload and 422", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodPost, "/rpc/demo.detail", bytes.NewBufferString(`null`))
		req.Header.Set("Content-Type", "application/json")
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, req)
		if rec.Code != http.StatusUnprocessableEntity {
			t.Fatalf("unexpected status %d body=%s", rec.Code, rec.Body.String())
		}

		var payload map[string]any
		if err := json.Unmarshal(rec.Body.Bytes(), &payload); err != nil {
			t.Fatalf("decode detail response: %v", err)
		}
		if got, _ := payload["type"].(string); got != "Detail" {
			t.Fatalf("unexpected error type %#v", payload)
		}
		detail, _ := payload["detail"].(map[string]any)
		if got, _ := detail["field"].(string); got != "title" {
			t.Fatalf("unexpected detail payload %#v", payload)
		}
	})
}

func TestDecodeValuesComplexShapesAndValidation(t *testing.T) {
	t.Run("complex query values bind slices maps pointers and numbers", func(t *testing.T) {
		values := url.Values{
			"single":            {"only"},
			"names":             {"red", "blue"},
			"counts[ok]":        {"7"},
			"counts[warn]":      {"9"},
			"score":             {"1.5"},
			"limit":             {"42"},
			"item[name]":        {"primary"},
			"item[enabled]":     {"true"},
			"items[0][name]":    {"first"},
			"items[0][enabled]": {"true"},
			"items[1][name]":    {"second"},
			"items[1][enabled]": {"false"},
			"meta[latency]":     {"2.25"},
		}

		var input complexDecodeInput
		if err := decodeValues(reflect.ValueOf(&input).Elem(), values); err != nil {
			t.Fatalf("decode values: %v", err)
		}

		if len(input.Single) != 1 || input.Single[0] != "only" {
			t.Fatalf("unexpected single %#v", input.Single)
		}
		if strings.Join(input.Names, ",") != "red,blue" {
			t.Fatalf("unexpected names %#v", input.Names)
		}
		if input.Counts["ok"] != 7 || input.Counts["warn"] != 9 {
			t.Fatalf("unexpected counts %#v", input.Counts)
		}
		if input.Score != 1.5 || input.Limit != 42 {
			t.Fatalf("unexpected numeric fields %#v", input)
		}
		if input.Item == nil || input.Item.Name != "primary" || !input.Item.Enabled {
			t.Fatalf("unexpected pointer item %#v", input.Item)
		}
		if len(input.Items) != 2 || input.Items[0].Name != "first" || input.Items[1].Enabled {
			t.Fatalf("unexpected items %#v", input.Items)
		}
		if got := input.Meta["latency"]; got != 2.25 {
			t.Fatalf("unexpected meta %#v", input.Meta)
		}
	})

	t.Run("missing nested required fields are rejected", func(t *testing.T) {
		values := url.Values{
			"item[name]": {"primary"},
		}

		var input nestedRequiredWrapper
		err := decodeValues(reflect.ValueOf(&input).Elem(), values)
		if err == nil || !strings.Contains(err.Error(), "missing field `enabled`") {
			t.Fatalf("expected nested missing field error, got %v", err)
		}
	})

	t.Run("non-struct wrappers and malformed keys are rejected", func(t *testing.T) {
		var number int
		if err := decodeValues(reflect.ValueOf(&number).Elem(), url.Values{"value": {"1"}}); err == nil || !strings.Contains(err.Error(), "struct wrappers") {
			t.Fatalf("expected struct wrapper error, got %v", err)
		}

		var input complexDecodeInput
		err := decodeValues(reflect.ValueOf(&input).Elem(), url.Values{"items[[broken": {"x"}})
		if err == nil || !strings.Contains(err.Error(), "malformed query key") {
			t.Fatalf("expected malformed key error, got %v", err)
		}
	})
}

func TestBindInputAndHelperEdgeCases(t *testing.T) {
	t.Run("bind input supports none and json error propagation", func(t *testing.T) {
		unit, err := bindInput(httptest.NewRequest(http.MethodGet, "/", nil), reflect.TypeOf(authMessage{}), teleport.InputNone)
		if err != nil {
			t.Fatalf("unexpected input none error: %v", err)
		}
		if _, ok := unit.(teleport.Unit); !ok {
			t.Fatalf("expected unit input, got %#v", unit)
		}

		req := httptest.NewRequest(http.MethodPost, "/", bytes.NewBufferString(`{"user":1}`))
		req.Header.Set("Content-Type", "application/json")
		_, err = bindInput(req, reflect.TypeOf(authMessage{}), teleport.InputJSONBody)
		if err == nil {
			t.Fatal("expected json decoding error")
		}
	})

	t.Run("bind input accepts form json bodies", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodPost, "/", bytes.NewBufferString(`{"user":"demo"}`))
		req.Header.Set("Content-Type", "application/json; charset=utf-8")
		value, err := bindInput(req, reflect.TypeOf(authMessage{}), teleport.InputFormBody)
		if err != nil {
			t.Fatalf("bind form json: %v", err)
		}
		if got := value.(authMessage); got.User != "demo" {
			t.Fatalf("unexpected form json value %#v", got)
		}
	})

	t.Run("assign value and helper guards reject unsupported shapes", func(t *testing.T) {
		target := reflect.New(reflect.TypeOf(map[int]string{})).Elem()
		if err := assignValue(target, map[string]any{"1": "x"}); err != strconv.ErrSyntax {
			t.Fatalf("expected non-string map key syntax error, got %v", err)
		}

		if _, err := toSlice(123); err != strconv.ErrSyntax {
			t.Fatalf("expected toSlice syntax error, got %v", err)
		}

		if _, err := parseSegments(""); err == nil {
			t.Fatal("expected empty key parse error")
		}
	})

	t.Run("field names and unit checks match runtime expectations", func(t *testing.T) {
		type fieldNames struct {
			DefaultName string
			Renamed     string `json:"renamed,omitempty"`
			Skipped     string `json:"-"`
		}

		typ := reflect.TypeOf(fieldNames{})
		if got := fieldName(typ.Field(0)); got != "defaultName" {
			t.Fatalf("unexpected default field name %q", got)
		}
		if got := fieldName(typ.Field(1)); got != "renamed" {
			t.Fatalf("unexpected renamed field %q", got)
		}
		if got := fieldName(typ.Field(2)); got != "" {
			t.Fatalf("unexpected skipped field %q", got)
		}

		if !isUnit(nil) || !isUnit(teleport.Unit{}) || isUnit(struct{}{}) {
			t.Fatal("unexpected isUnit behavior")
		}
	})
}
