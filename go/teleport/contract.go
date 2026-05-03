// Package teleport contains the shared contract types and explicit procedure
// authoring helpers for the Go Teleport implementation.
package teleport

import (
	"encoding/json"
	"sort"
)

const ContractVersion = "teleport.contract/v1"

type ContractBundle struct {
	Version    string              `json:"version"`
	Procedures []ProcedureContract `json:"procedures"`
	Types      []NamedTypeContract `json:"types"`
}

func NewContractBundle() ContractBundle {
	return ContractBundle{
		Version:    ContractVersion,
		Procedures: []ProcedureContract{},
		Types:      []NamedTypeContract{},
	}
}

func (b *ContractBundle) Sort() {
	sort.Slice(b.Procedures, func(i, j int) bool { return b.Procedures[i].Name < b.Procedures[j].Name })
	sort.Slice(b.Types, func(i, j int) bool { return b.Types[i].Name < b.Types[j].Name })
}

type ProcedureContract struct {
	Name          string        `json:"name"`
	Namespace     string        `json:"namespace"`
	MethodName    string        `json:"method_name"`
	ProcedureKind ProcedureKind `json:"procedure_kind"`
	HTTPMethod    HTTPMethod    `json:"http_method"`
	Path          string        `json:"path"`
	InputEncoding InputEncoding `json:"input_encoding"`
	AuthMode      AuthMode      `json:"auth_mode"`
	Doc           string        `json:"doc"`
	InputType     TypeExpr      `json:"input_type"`
	OutputType    TypeExpr      `json:"output_type"`
	ErrorType     TypeExpr      `json:"error_type"`
}

type ProcedureKind string

const (
	ProcedureQuery   ProcedureKind = "Query"
	ProcedureCommand ProcedureKind = "Command"
	ProcedureForm    ProcedureKind = "Form"
)

type HTTPMethod string

const (
	MethodGet  HTTPMethod = "Get"
	MethodPost HTTPMethod = "Post"
)

func (m HTTPMethod) String() string {
	switch m {
	case MethodGet:
		return "GET"
	case MethodPost:
		return "POST"
	default:
		return ""
	}
}

type InputEncoding string

const (
	InputNone        InputEncoding = "None"
	InputQueryString InputEncoding = "QueryString"
	InputJSONBody    InputEncoding = "JsonBody"
	InputFormBody    InputEncoding = "FormBody"
)

type AuthMode string

const (
	AuthNone     AuthMode = "None"
	AuthRequired AuthMode = "Required"
	AuthOptional AuthMode = "Optional"
)

type PrimitiveType string

const (
	PrimitiveI8    PrimitiveType = "i8"
	PrimitiveI16   PrimitiveType = "i16"
	PrimitiveI32   PrimitiveType = "i32"
	PrimitiveI64   PrimitiveType = "i64"
	PrimitiveI128  PrimitiveType = "i128"
	PrimitiveIsize PrimitiveType = "isize"
	PrimitiveU8    PrimitiveType = "u8"
	PrimitiveU16   PrimitiveType = "u16"
	PrimitiveU32   PrimitiveType = "u32"
	PrimitiveU64   PrimitiveType = "u64"
	PrimitiveU128  PrimitiveType = "u128"
	PrimitiveUsize PrimitiveType = "usize"
	PrimitiveF16   PrimitiveType = "f16"
	PrimitiveF32   PrimitiveType = "f32"
	PrimitiveF64   PrimitiveType = "f64"
	PrimitiveF128  PrimitiveType = "f128"
	PrimitiveBool  PrimitiveType = "bool"
	PrimitiveChar  PrimitiveType = "char"
	PrimitiveStr   PrimitiveType = "str"
)

type TypeExpr struct {
	tag   string
	value any
}

func PrimitiveExpr(value PrimitiveType) TypeExpr { return TypeExpr{tag: "Primitive", value: value} }
func ListExpr(value TypeExpr) TypeExpr           { return TypeExpr{tag: "List", value: value} }
func MapExpr(key, value TypeExpr) TypeExpr {
	return TypeExpr{tag: "Map", value: map[string]TypeExpr{"key": key, "value": value}}
}
func TupleExpr(values ...TypeExpr) TypeExpr {
	if values == nil {
		values = []TypeExpr{}
	}
	return TypeExpr{tag: "Tuple", value: values}
}
func NullableExpr(value TypeExpr) TypeExpr { return TypeExpr{tag: "Nullable", value: value} }
func NamedExpr(name string, generics ...TypeExpr) TypeExpr {
	if generics == nil {
		generics = []TypeExpr{}
	}
	return TypeExpr{tag: "Named", value: map[string]any{"name": name, "generics": generics}}
}
func GenericExpr(name string) TypeExpr { return TypeExpr{tag: "Generic", value: name} }
func OpaqueExpr(name string) TypeExpr  { return TypeExpr{tag: "Opaque", value: name} }

func (expr TypeExpr) MarshalJSON() ([]byte, error) {
	return json.Marshal(map[string]any{expr.tag: expr.value})
}

type NamedTypeContract struct {
	Name     string        `json:"name"`
	Docs     string        `json:"docs"`
	Generics []string      `json:"generics"`
	Kind     NamedTypeKind `json:"kind"`
}

func StructType(name string, fields FieldsContract) NamedTypeContract {
	return NamedTypeContract{Name: name, Docs: "", Generics: []string{}, Kind: StructKind(fields)}
}

type NamedTypeKind struct {
	tag   string
	value any
}

func StructKind(fields FieldsContract) NamedTypeKind {
	return NamedTypeKind{tag: "Struct", value: fields}
}
func EnumKind(variants ...VariantContract) NamedTypeKind {
	return NamedTypeKind{tag: "Enum", value: variants}
}
func AliasKind(value TypeExpr) NamedTypeKind { return NamedTypeKind{tag: "Alias", value: value} }

func (kind NamedTypeKind) MarshalJSON() ([]byte, error) {
	return json.Marshal(map[string]any{kind.tag: kind.value})
}

type FieldsContract struct {
	tag   string
	value any
}

func UnitFields() FieldsContract { return FieldsContract{tag: "Unit", value: []any{}} }
func NamedFields(fields ...NamedFieldContract) FieldsContract {
	return FieldsContract{tag: "Named", value: fields}
}
func UnnamedFields(fields ...UnnamedFieldContract) FieldsContract {
	return FieldsContract{tag: "Unnamed", value: fields}
}

func (fields FieldsContract) MarshalJSON() ([]byte, error) {
	if fields.tag == "Unit" {
		return json.Marshal("Unit")
	}
	return json.Marshal(map[string]any{fields.tag: fields.value})
}

type NamedFieldContract struct {
	Name     string    `json:"name"`
	Docs     string    `json:"docs"`
	Optional bool      `json:"optional"`
	Ty       *TypeExpr `json:"ty"`
}

func Field(name string, ty TypeExpr) NamedFieldContract {
	return NamedFieldContract{Name: name, Docs: "", Optional: false, Ty: &ty}
}

func OptionalField(name string, ty TypeExpr) NamedFieldContract {
	return NamedFieldContract{Name: name, Docs: "", Optional: true, Ty: &ty}
}

type UnnamedFieldContract struct {
	Docs string    `json:"docs"`
	Ty   *TypeExpr `json:"ty"`
}

type VariantContract struct {
	Name   string         `json:"name"`
	Docs   string         `json:"docs"`
	Fields FieldsContract `json:"fields"`
}
