package teleport

import (
	"reflect"
	"sort"
	"strings"
)

var unitType = reflect.TypeFor[Unit]()

func exportTypeExpr(typ reflect.Type) TypeExpr {
	if typ == nil || typ == unitType {
		return TupleExpr()
	}
	if isNamedContractType(typ) {
		return NamedExpr(typ.Name())
	}
	if typ.Kind() == reflect.Pointer {
		return NullableExpr(exportTypeExpr(typ.Elem()))
	}
	return exportUnnamedTypeExpr(typ)
}

func exportUnnamedTypeExpr(typ reflect.Type) TypeExpr {
	if typ == nil || typ == unitType {
		return TupleExpr()
	}

	switch typ.Kind() {
	case reflect.Bool:
		return PrimitiveExpr(PrimitiveBool)
	case reflect.Int8:
		return PrimitiveExpr(PrimitiveI8)
	case reflect.Int16:
		return PrimitiveExpr(PrimitiveI16)
	case reflect.Int32:
		return PrimitiveExpr(PrimitiveI32)
	case reflect.Int, reflect.Int64:
		return PrimitiveExpr(PrimitiveI64)
	case reflect.Uint8:
		return PrimitiveExpr(PrimitiveU8)
	case reflect.Uint16:
		return PrimitiveExpr(PrimitiveU16)
	case reflect.Uint32:
		return PrimitiveExpr(PrimitiveU32)
	case reflect.Uint, reflect.Uint64:
		return PrimitiveExpr(PrimitiveU64)
	case reflect.Float32:
		return PrimitiveExpr(PrimitiveF32)
	case reflect.Float64:
		return PrimitiveExpr(PrimitiveF64)
	case reflect.String:
		return PrimitiveExpr(PrimitiveStr)
	case reflect.Pointer:
		return NullableExpr(exportTypeExpr(typ.Elem()))
	case reflect.Slice, reflect.Array:
		return ListExpr(exportTypeExpr(typ.Elem()))
	case reflect.Map:
		return MapExpr(exportTypeExpr(typ.Key()), exportTypeExpr(typ.Elem()))
	case reflect.Struct:
		return OpaqueExpr("anonymous_struct")
	default:
		return OpaqueExpr(typ.String())
	}
}

// ExportNamedTypes exports named Go types and the named types they reference.
func ExportNamedTypes(types ...reflect.Type) []NamedTypeContract {
	queue := append([]reflect.Type(nil), types...)
	seen := map[string]struct{}{}
	result := []NamedTypeContract{}

	for len(queue) > 0 {
		typ := exportRootType(queue[0])
		queue = queue[1:]
		if !isNamedContractType(typ) {
			continue
		}

		key := typeKey(typ)
		if _, ok := seen[key]; ok {
			continue
		}
		seen[key] = struct{}{}

		contract, refs := exportNamedType(typ)
		result = append(result, contract)
		queue = append(queue, refs...)
	}

	sort.Slice(result, func(i, j int) bool {
		return result[i].Name < result[j].Name
	})
	return result
}

func exportNamedType(typ reflect.Type) (NamedTypeContract, []reflect.Type) {
	refs := []reflect.Type{}
	track := func(ref reflect.Type) {
		ref = exportRootType(ref)
		if isNamedContractType(ref) {
			refs = append(refs, ref)
		}
	}

	switch typ.Kind() {
	case reflect.Struct:
		fields := []NamedFieldContract{}
		for i := range typ.NumField() {
			field := typ.Field(i)
			if field.PkgPath != "" {
				continue
			}
			name, include := jsonFieldName(field)
			if !include {
				continue
			}
			collectReferencedNamedTypes(field.Type, track)
			expr := exportTypeExpr(field.Type)
			fields = append(fields, NamedFieldContract{
				Name:     name,
				Docs:     "",
				Optional: false,
				Ty:       &expr,
			})
		}
		return StructType(typ.Name(), NamedFields(fields...)), refs
	default:
		collectAliasReferencedNamedTypes(typ, track)
		return NamedTypeContract{
			Name:     typ.Name(),
			Docs:     "",
			Generics: []string{},
			Kind:     AliasKind(exportAliasExpr(typ)),
		}, refs
	}
}

func exportAliasExpr(typ reflect.Type) TypeExpr {
	switch typ.Kind() {
	case reflect.Pointer:
		return NullableExpr(exportTypeExpr(typ.Elem()))
	case reflect.Slice, reflect.Array:
		return ListExpr(exportTypeExpr(typ.Elem()))
	case reflect.Map:
		return MapExpr(exportTypeExpr(typ.Key()), exportTypeExpr(typ.Elem()))
	default:
		return exportUnnamedTypeExpr(typ)
	}
}

func collectReferencedNamedTypes(typ reflect.Type, visit func(reflect.Type)) {
	if typ == nil || typ == unitType {
		return
	}

	if isNamedContractType(typ) {
		visit(typ)
		if typ.Kind() == reflect.Struct {
			for i := range typ.NumField() {
				field := typ.Field(i)
				if field.PkgPath != "" {
					continue
				}
				if _, include := jsonFieldName(field); !include {
					continue
				}
				collectReferencedNamedTypes(field.Type, visit)
			}
			return
		}
		collectAliasReferencedNamedTypes(typ, visit)
		return
	}

	switch typ.Kind() {
	case reflect.Pointer, reflect.Slice, reflect.Array:
		collectReferencedNamedTypes(typ.Elem(), visit)
	case reflect.Map:
		collectReferencedNamedTypes(typ.Key(), visit)
		collectReferencedNamedTypes(typ.Elem(), visit)
	case reflect.Struct:
		for i := range typ.NumField() {
			field := typ.Field(i)
			if field.PkgPath != "" {
				continue
			}
			if _, include := jsonFieldName(field); !include {
				continue
			}
			collectReferencedNamedTypes(field.Type, visit)
		}
	}
}

func collectAliasReferencedNamedTypes(typ reflect.Type, visit func(reflect.Type)) {
	switch typ.Kind() {
	case reflect.Pointer, reflect.Slice, reflect.Array:
		collectReferencedNamedTypes(typ.Elem(), visit)
	case reflect.Map:
		collectReferencedNamedTypes(typ.Key(), visit)
		collectReferencedNamedTypes(typ.Elem(), visit)
	}
}

func exportRootType(typ reflect.Type) reflect.Type {
	for typ != nil && typ != unitType && typ.Kind() == reflect.Pointer && !isNamedContractType(typ) {
		typ = typ.Elem()
	}
	return typ
}

func isNamedContractType(typ reflect.Type) bool {
	return typ != nil && typ != unitType && typ.Name() != "" && typ.PkgPath() != ""
}

func typeKey(typ reflect.Type) string {
	return typ.PkgPath() + "." + typ.Name()
}

func jsonFieldName(field reflect.StructField) (string, bool) {
	tag := field.Tag.Get("json")
	if tag == "-" {
		return "", false
	}
	if tag == "" {
		return strings.ToLower(field.Name[:1]) + field.Name[1:], true
	}
	name := strings.Split(tag, ",")[0]
	if name == "" {
		return strings.ToLower(field.Name[:1]) + field.Name[1:], true
	}
	return name, true
}
