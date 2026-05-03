package teleport

import (
	"encoding/json"
	"reflect"
	"strings"
	"testing"
)

type exportChild struct {
	Label string `json:"label"`
}

type exportParent struct {
	Child      exportChild            `json:"child"`
	Children   []exportChild          `json:"children"`
	Lookup     map[string]exportChild `json:"lookup"`
	Optional   *exportChild           `json:"optional,omitempty"`
	DefaultTag string
	Ignored    string `json:"-"`
}

type exportChildList []exportChild
type exportChildMap map[string]exportChild
type exportChildPtr *exportChild
type exportCount int32

func TestExportUnnamedAndNamedTypeContracts(t *testing.T) {
	t.Run("unnamed expressions cover primitives and aggregates", func(t *testing.T) {
		cases := []struct {
			typ  reflect.Type
			want string
		}{
			{reflect.TypeOf(true), `{"Primitive":"bool"}`},
			{reflect.TypeOf(int16(1)), `{"Primitive":"i16"}`},
			{reflect.TypeOf(uint32(1)), `{"Primitive":"u32"}`},
			{reflect.TypeOf(float64(1)), `{"Primitive":"f64"}`},
			{reflect.TypeOf("x"), `{"Primitive":"str"}`},
			{reflect.TypeOf(&exportChild{}), `{"Nullable":{"Named":{"generics":[],"name":"exportChild"}}}`},
			{reflect.TypeOf([]exportChild{}), `{"List":{"Named":{"generics":[],"name":"exportChild"}}}`},
			{reflect.TypeOf(map[string]exportChild{}), `{"Map":{"key":{"Primitive":"str"},"value":{"Named":{"generics":[],"name":"exportChild"}}}}`},
			{reflect.TypeOf(struct{ Hidden int }{}), `{"Opaque":"anonymous_struct"}`},
			{reflect.TypeOf(make(chan int)), `{"Opaque":"chan int"}`},
		}

		for _, tc := range cases {
			body, err := json.Marshal(exportUnnamedTypeExpr(tc.typ))
			if err != nil {
				t.Fatalf("marshal type expr: %v", err)
			}
			if got := string(body); got != tc.want {
				t.Fatalf("unexpected expr for %v: %s", tc.typ, got)
			}
		}
	})

	t.Run("named exports include recursive references and alias shapes", func(t *testing.T) {
		contracts := ExportNamedTypes(
			reflect.TypeOf(&exportParent{}),
			reflect.TypeOf(exportChildList{}),
			reflect.TypeOf(exportChildMap{}),
			reflect.TypeOf(exportChildPtr(nil)),
			reflect.TypeOf(exportCount(0)),
		)

		body, err := json.Marshal(contracts)
		if err != nil {
			t.Fatalf("marshal named types: %v", err)
		}
		jsonBody := string(body)

		for _, want := range []string{
			`"name":"exportChild"`,
			`"name":"exportParent"`,
			`"name":"exportChildList","docs":"","generics":[],"kind":{"Alias":{"List":{"Named":{"generics":[],"name":"exportChild"}}}}`,
			`"name":"exportChildMap","docs":"","generics":[],"kind":{"Alias":{"Map":{"key":{"Primitive":"str"},"value":{"Named":{"generics":[],"name":"exportChild"}}}}}`,
			`"name":"exportChildPtr","docs":"","generics":[],"kind":{"Alias":{"Nullable":{"Named":{"generics":[],"name":"exportChild"}}}}`,
			`"name":"exportCount","docs":"","generics":[],"kind":{"Alias":{"Primitive":"i32"}}`,
			`"name":"defaultTag"`,
		} {
			if !strings.Contains(jsonBody, want) {
				t.Fatalf("expected %q in %s", want, jsonBody)
			}
		}
		if strings.Contains(jsonBody, `"Ignored"`) || strings.Contains(jsonBody, `"ignored"`) {
			t.Fatalf("ignored field leaked into contract: %s", jsonBody)
		}
	})

	t.Run("json field naming follows tags", func(t *testing.T) {
		type fields struct {
			DefaultName string
			Renamed     string `json:"renamed,omitempty"`
			Embedded    string `json:",omitempty"`
			Skipped     string `json:"-"`
		}

		fieldCases := map[string]struct {
			index int
			name  string
			ok    bool
		}{
			"default": {index: 0, name: "defaultName", ok: true},
			"renamed": {index: 1, name: "renamed", ok: true},
			"empty":   {index: 2, name: "embedded", ok: true},
			"skipped": {index: 3, name: "", ok: false},
		}

		typ := reflect.TypeOf(fields{})
		for _, tc := range fieldCases {
			name, ok := jsonFieldName(typ.Field(tc.index))
			if name != tc.name || ok != tc.ok {
				t.Fatalf("unexpected field name result index=%d got=(%q,%v)", tc.index, name, ok)
			}
		}
	})
}
