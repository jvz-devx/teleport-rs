// Package teleporthttp exposes Teleport procedures through net/http.
package teleporthttp

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"reflect"
	"strconv"
	"strings"

	"github.com/jvz-devx/teleport-rs/go/teleport"
)

// Router registers Teleport procedures and serves them through net/http.
type Router struct {
	authenticator func(*http.Request) (any, bool)
	procedures    []teleport.Procedure
	types         []teleport.NamedTypeContract
	manifest      bool
}

// New creates an empty Teleport HTTP router.
func New() *Router {
	return &Router{
		procedures: []teleport.Procedure{},
		types:      []teleport.NamedTypeContract{},
	}
}

// SetAuthenticator configures the auth hook used for required and optional auth procedures.
func (r *Router) SetAuthenticator(fn func(*http.Request) (any, bool)) *Router {
	r.authenticator = fn
	return r
}

// IncludeManifest enables or disables GET /rpc/__manifest.
func (r *Router) IncludeManifest(enabled bool) *Router {
	r.manifest = enabled
	return r
}

// AddTypes appends named contract types exported by the application.
func (r *Router) AddTypes(types ...teleport.NamedTypeContract) *Router {
	r.types = append(r.types, types...)
	return r
}

// Register appends procedures to the router.
func (r *Router) Register(procedures ...teleport.Procedure) *Router {
	r.procedures = append(r.procedures, procedures...)
	return r
}

// Contract returns the sorted language-neutral contract for the registered procedures and types.
func (r *Router) Contract() teleport.ContractBundle {
	bundle := teleport.NewContractBundle()
	for _, procedure := range r.procedures {
		bundle.Procedures = append(bundle.Procedures, procedure.Contract)
	}
	bundle.Types = append(bundle.Types, r.types...)
	bundle.Sort()
	return bundle
}

// ServeHTTP implements http.Handler.
func (r *Router) ServeHTTP(w http.ResponseWriter, req *http.Request) {
	if r.manifest && req.URL.Path == "/rpc/__manifest" && req.Method == http.MethodGet {
		writeJSON(w, http.StatusOK, manifestFromBundle(r.Contract()))
		return
	}

	for _, procedure := range r.procedures {
		if req.URL.Path != procedure.Contract.Path {
			continue
		}
		if req.Method != procedure.Contract.HTTPMethod.String() {
			w.WriteHeader(http.StatusMethodNotAllowed)
			return
		}
		r.serveProcedure(w, req, procedure)
		return
	}

	http.NotFound(w, req)
}

func (r *Router) serveProcedure(w http.ResponseWriter, req *http.Request, procedure teleport.Procedure) {
	defer func() {
		if recovered := recover(); recovered != nil {
			writeJSON(w, http.StatusInternalServerError, teleport.InternalError("internal server error"))
		}
	}()

	var user any
	if procedure.Contract.AuthMode != teleport.AuthNone {
		if r.authenticator == nil {
			if procedure.Contract.AuthMode == teleport.AuthOptional {
				user = nil
			} else {
				writeJSON(w, http.StatusUnauthorized, teleport.UnauthorizedError())
				return
			}
		} else {
			resolved, ok := r.authenticator(req)
			if !ok && procedure.Contract.AuthMode == teleport.AuthRequired {
				writeJSON(w, http.StatusUnauthorized, teleport.UnauthorizedError())
				return
			}
			user = resolved
		}
	}

	input, err := bindInput(req, procedure.InputType, procedure.Contract.InputEncoding)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, teleport.BadRequestError(err.Error()))
		return
	}

	ok, value, appErr := procedure.Invoke(teleport.RequestContext{
		Request: req,
		Writer:  w,
		User:    user,
	}, input)
	if !ok {
		writeJSON(w, appErr.StatusCode(), appErr)
		return
	}

	if isUnit(value) {
		writeJSON(w, http.StatusOK, nil)
		return
	}
	writeJSON(w, http.StatusOK, value)
}

func manifestFromBundle(bundle teleport.ContractBundle) map[string]any {
	procedures := map[string]any{}
	for _, procedure := range bundle.Procedures {
		procedures[procedure.Name] = map[string]any{
			"method": procedure.HTTPMethod.String(),
			"path":   procedure.Path,
		}
	}
	return map[string]any{"procedures": procedures}
}

func bindInput(req *http.Request, typ reflect.Type, encoding teleport.InputEncoding) (any, error) {
	if typ == reflect.TypeOf(teleport.Unit{}) {
		return teleport.Unit{}, nil
	}

	value := reflect.New(typ)
	switch encoding {
	case teleport.InputNone:
		return teleport.Unit{}, nil
	case teleport.InputJSONBody:
		if err := json.NewDecoder(req.Body).Decode(value.Interface()); err != nil {
			return nil, err
		}
	case teleport.InputFormBody:
		if strings.HasPrefix(req.Header.Get("Content-Type"), "application/json") {
			if err := json.NewDecoder(req.Body).Decode(value.Interface()); err != nil {
				return nil, err
			}
		} else {
			if err := req.ParseForm(); err != nil {
				return nil, err
			}
			if err := decodeValues(value.Elem(), req.PostForm); err != nil {
				return nil, err
			}
		}
	case teleport.InputQueryString:
		if err := decodeValues(value.Elem(), req.URL.Query()); err != nil {
			return nil, err
		}
	}
	return value.Elem().Interface(), nil
}

func decodeValues(target reflect.Value, values url.Values) error {
	if target.Kind() != reflect.Struct {
		return fmt.Errorf("query and form inputs must be struct wrappers")
	}

	root := map[string]any{}
	for key, items := range values {
		for _, item := range items {
			if err := insertValue(root, key, item); err != nil {
				return err
			}
		}
	}

	if err := assignValue(target, root); err != nil {
		return err
	}
	return validateRequiredFields(target.Type(), root)
}

func insertValue(root map[string]any, key, value string) error {
	segments, err := parseSegments(key)
	if err != nil {
		return err
	}

	return insertNode(root, segments, 0, value)
}

func insertNode(container any, segments []pathSegment, index int, value string) error {
	segment := segments[index]
	last := index == len(segments)-1

	switch current := segment.(type) {
	case propertySegment:
		obj, ok := container.(map[string]any)
		if !ok {
			return strconv.ErrSyntax
		}
		if last {
			addProperty(obj, current.name, value)
			return nil
		}

		child, ok := obj[current.name]
		if !ok || child == nil {
			child = createContainer(segments[index+1])
			obj[current.name] = child
		}
		return insertNode(child, segments, index+1, value)
	case indexSegment:
		array, ok := container.(*[]any)
		if !ok {
			return strconv.ErrSyntax
		}
		for len(*array) <= current.value {
			*array = append(*array, nil)
		}
		if last {
			(*array)[current.value] = value
			return nil
		}

		child := (*array)[current.value]
		if child == nil {
			child = createContainer(segments[index+1])
			(*array)[current.value] = child
		}
		return insertNode(child, segments, index+1, value)
	case appendSegment:
		array, ok := container.(*[]any)
		if !ok {
			return strconv.ErrSyntax
		}
		if last {
			*array = append(*array, value)
			return nil
		}

		child := createContainer(segments[index+1])
		*array = append(*array, child)
		return insertNode(child, segments, index+1, value)
	default:
		return strconv.ErrSyntax
	}
}

func addProperty(obj map[string]any, name, value string) {
	existing, ok := obj[name]
	if !ok || existing == nil {
		obj[name] = value
		return
	}

	switch typed := existing.(type) {
	case *[]any:
		*typed = append(*typed, value)
	case []any:
		obj[name] = append(typed, value)
	default:
		obj[name] = []any{typed, value}
	}
}

func createContainer(segment pathSegment) any {
	switch segment.(type) {
	case propertySegment:
		return map[string]any{}
	case indexSegment, appendSegment:
		items := []any{}
		return &items
	default:
		return map[string]any{}
	}
}

func assignValue(target reflect.Value, raw any) error {
	if !target.CanSet() {
		return strconv.ErrSyntax
	}

	if target.Kind() == reflect.Pointer {
		if raw == nil {
			return nil
		}
		target.Set(reflect.New(target.Type().Elem()))
		return assignValue(target.Elem(), raw)
	}

	switch target.Kind() {
	case reflect.Struct:
		obj, ok := raw.(map[string]any)
		if !ok {
			return strconv.ErrSyntax
		}
		for i := range target.NumField() {
			field := target.Type().Field(i)
			if field.PkgPath != "" {
				continue
			}
			rawField, ok := obj[fieldName(field)]
			if !ok {
				continue
			}
			if err := assignValue(target.Field(i), rawField); err != nil {
				return err
			}
		}
		return nil
	case reflect.Slice:
		items, err := toSlice(raw)
		if err != nil {
			return err
		}
		slice := reflect.MakeSlice(target.Type(), len(items), len(items))
		for i, item := range items {
			if err := assignValue(slice.Index(i), item); err != nil {
				return err
			}
		}
		target.Set(slice)
		return nil
	case reflect.Map:
		if target.Type().Key().Kind() != reflect.String {
			return strconv.ErrSyntax
		}
		obj, ok := raw.(map[string]any)
		if !ok {
			return strconv.ErrSyntax
		}
		result := reflect.MakeMapWithSize(target.Type(), len(obj))
		for key, value := range obj {
			entry := reflect.New(target.Type().Elem()).Elem()
			if err := assignValue(entry, value); err != nil {
				return err
			}
			result.SetMapIndex(reflect.ValueOf(key).Convert(target.Type().Key()), entry)
		}
		target.Set(result)
		return nil
	case reflect.String:
		text, ok := raw.(string)
		if !ok {
			return strconv.ErrSyntax
		}
		target.SetString(text)
		return nil
	case reflect.Bool:
		text, ok := raw.(string)
		if !ok {
			return strconv.ErrSyntax
		}
		value, err := strconv.ParseBool(text)
		if err != nil {
			return err
		}
		target.SetBool(value)
		return nil
	case reflect.Int, reflect.Int64, reflect.Int32, reflect.Int16, reflect.Int8:
		text, ok := raw.(string)
		if !ok {
			return strconv.ErrSyntax
		}
		value, err := strconv.ParseInt(text, 10, target.Type().Bits())
		if err != nil {
			return err
		}
		target.SetInt(value)
		return nil
	case reflect.Uint, reflect.Uint64, reflect.Uint32, reflect.Uint16, reflect.Uint8:
		text, ok := raw.(string)
		if !ok {
			return strconv.ErrSyntax
		}
		value, err := strconv.ParseUint(text, 10, target.Type().Bits())
		if err != nil {
			return err
		}
		target.SetUint(value)
		return nil
	case reflect.Float32, reflect.Float64:
		text, ok := raw.(string)
		if !ok {
			return strconv.ErrSyntax
		}
		value, err := strconv.ParseFloat(text, target.Type().Bits())
		if err != nil {
			return err
		}
		target.SetFloat(value)
		return nil
	default:
		return strconv.ErrSyntax
	}
}

func toSlice(raw any) ([]any, error) {
	switch typed := raw.(type) {
	case *[]any:
		return *typed, nil
	case []any:
		return typed, nil
	case string:
		return []any{typed}, nil
	default:
		return nil, strconv.ErrSyntax
	}
}

func fieldName(field reflect.StructField) string {
	tag := field.Tag.Get("json")
	if tag == "" {
		return strings.ToLower(field.Name[:1]) + field.Name[1:]
	}
	name := strings.Split(tag, ",")[0]
	if name == "-" {
		return ""
	}
	if name == "" {
		return strings.ToLower(field.Name[:1]) + field.Name[1:]
	}
	return name
}

func validateRequiredFields(typ reflect.Type, raw any) error {
	for typ.Kind() == reflect.Pointer {
		typ = typ.Elem()
	}

	switch typ.Kind() {
	case reflect.Struct:
		obj, ok := raw.(map[string]any)
		if !ok {
			return strconv.ErrSyntax
		}
		for i := range typ.NumField() {
			field := typ.Field(i)
			if field.PkgPath != "" {
				continue
			}
			name := fieldName(field)
			if name == "" {
				continue
			}
			value, ok := obj[name]
			if !ok {
				if isOptionalField(field.Type) {
					continue
				}
				return fmt.Errorf("missing field `%s`", name)
			}
			if err := validateRequiredFields(field.Type, value); err != nil {
				return err
			}
		}
	case reflect.Slice, reflect.Array:
		switch typed := raw.(type) {
		case []any:
			for _, item := range typed {
				if item == nil {
					continue
				}
				if err := validateRequiredFields(typ.Elem(), item); err != nil {
					return err
				}
			}
		case *[]any:
			for _, item := range *typed {
				if item == nil {
					continue
				}
				if err := validateRequiredFields(typ.Elem(), item); err != nil {
					return err
				}
			}
		}
	}

	return nil
}

func isOptionalField(typ reflect.Type) bool {
	return typ.Kind() == reflect.Pointer
}

func parseSegments(key string) ([]pathSegment, error) {
	if key == "" {
		return nil, fmt.Errorf("empty query key")
	}

	segments := []pathSegment{}
	var current strings.Builder

	for i := 0; i < len(key); i++ {
		switch key[i] {
		case '[':
			if current.Len() > 0 {
				segments = append(segments, propertySegment{name: current.String()})
				current.Reset()
			}

			close := strings.IndexByte(key[i+1:], ']')
			if close < 0 {
				return nil, fmt.Errorf("malformed query key %q", key)
			}
			inside := key[i+1 : i+1+close]
			switch {
			case inside == "":
				segments = append(segments, appendSegment{})
			default:
				index, err := strconv.Atoi(inside)
				if err == nil {
					segments = append(segments, indexSegment{value: index})
				} else {
					segments = append(segments, propertySegment{name: inside})
				}
			}
			i += close + 1
		default:
			current.WriteByte(key[i])
		}
	}

	if current.Len() > 0 {
		segments = append(segments, propertySegment{name: current.String()})
	}

	if len(segments) == 0 {
		return nil, fmt.Errorf("malformed query key %q", key)
	}
	return segments, nil
}

type pathSegment interface {
	isPathSegment()
}

type propertySegment struct {
	name string
}

func (propertySegment) isPathSegment() {}

type indexSegment struct {
	value int
}

func (indexSegment) isPathSegment() {}

type appendSegment struct{}

func (appendSegment) isPathSegment() {}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}

func isUnit(value any) bool {
	if value == nil {
		return true
	}
	return reflect.TypeOf(value) == reflect.TypeOf(teleport.Unit{})
}
