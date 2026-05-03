package teleport

import (
	"fmt"
	"net/http"
	"reflect"
)

// RequestContext is passed to every procedure handler.
// It carries the original HTTP request, response writer, and authenticated user.
type RequestContext struct {
	Request *http.Request
	Writer  http.ResponseWriter
	User    any
}

// Procedure is a registered remote procedure plus its exported contract shape.
type Procedure struct {
	Contract   ProcedureContract
	InputType  reflect.Type
	OutputType reflect.Type
	ErrorType  reflect.Type
	invoke     func(RequestContext, any) (bool, any, *AppError)
}

// ProcedureBuilder collects procedure metadata before the typed handler is attached.
type ProcedureBuilder[TIn any, TOut any, TErr any] struct {
	namespace string
	method    string
	kind      ProcedureKind
	auth      AuthMode
	doc       string
}

// Invoke calls the procedure handler with a decoded input value.
func (p Procedure) Invoke(ctx RequestContext, input any) (bool, any, *AppError) {
	return p.invoke(ctx, input)
}

// WithDoc returns a copy of the procedure with contract documentation attached.
func (p Procedure) WithDoc(doc string) Procedure {
	p.Contract.Doc = doc
	return p
}

// QueryFor starts a GET procedure builder that reads input from the query string.
func QueryFor[TIn any, TOut any](namespace string, method string) ProcedureBuilder[TIn, TOut, Unit] {
	return QueryWithErrorFor[TIn, TOut, Unit](namespace, method)
}

// QueryWithErrorFor starts a GET procedure builder with a typed Detail error payload.
func QueryWithErrorFor[TIn any, TOut any, TErr any](namespace string, method string) ProcedureBuilder[TIn, TOut, TErr] {
	return newProcedureBuilder[TIn, TOut, TErr](namespace, method, ProcedureQuery)
}

// Query creates a GET procedure that reads input from the query string.
func Query[TIn any, TOut any](
	namespace string,
	method string,
	auth AuthMode,
	handler func(RequestContext, TIn) Result[TOut],
) Procedure {
	return QueryFor[TIn, TOut](namespace, method).Auth(auth).Handle(handler)
}

// QueryWithError creates a GET procedure with a typed Detail error payload.
func QueryWithError[TIn any, TOut any, TErr any](
	namespace string,
	method string,
	auth AuthMode,
	handler func(RequestContext, TIn) Result[TOut],
) Procedure {
	return QueryWithErrorFor[TIn, TOut, TErr](namespace, method).Auth(auth).Handle(handler)
}

// CommandFor starts a POST procedure builder that reads input from a JSON request body.
func CommandFor[TIn any, TOut any](namespace string, method string) ProcedureBuilder[TIn, TOut, Unit] {
	return CommandWithErrorFor[TIn, TOut, Unit](namespace, method)
}

// CommandWithErrorFor starts a JSON POST procedure builder with a typed Detail error payload.
func CommandWithErrorFor[TIn any, TOut any, TErr any](namespace string, method string) ProcedureBuilder[TIn, TOut, TErr] {
	return newProcedureBuilder[TIn, TOut, TErr](namespace, method, ProcedureCommand)
}

// Command creates a POST procedure that reads input from a JSON request body.
func Command[TIn any, TOut any](
	namespace string,
	method string,
	auth AuthMode,
	handler func(RequestContext, TIn) Result[TOut],
) Procedure {
	return CommandFor[TIn, TOut](namespace, method).Auth(auth).Handle(handler)
}

// CommandWithError creates a JSON POST procedure with a typed Detail error payload.
func CommandWithError[TIn any, TOut any, TErr any](
	namespace string,
	method string,
	auth AuthMode,
	handler func(RequestContext, TIn) Result[TOut],
) Procedure {
	return CommandWithErrorFor[TIn, TOut, TErr](namespace, method).Auth(auth).Handle(handler)
}

// FormFor starts a POST procedure builder that reads input from multipart or URL-encoded form data.
func FormFor[TIn any, TOut any](namespace string, method string) ProcedureBuilder[TIn, TOut, Unit] {
	return FormWithErrorFor[TIn, TOut, Unit](namespace, method)
}

// FormWithErrorFor starts a form POST procedure builder with a typed Detail error payload.
func FormWithErrorFor[TIn any, TOut any, TErr any](namespace string, method string) ProcedureBuilder[TIn, TOut, TErr] {
	return newProcedureBuilder[TIn, TOut, TErr](namespace, method, ProcedureForm)
}

// Form creates a POST procedure that reads input from multipart or URL-encoded form data.
func Form[TIn any, TOut any](
	namespace string,
	method string,
	auth AuthMode,
	handler func(RequestContext, TIn) Result[TOut],
) Procedure {
	return FormFor[TIn, TOut](namespace, method).Auth(auth).Handle(handler)
}

// FormWithError creates a form POST procedure with a typed Detail error payload.
func FormWithError[TIn any, TOut any, TErr any](
	namespace string,
	method string,
	auth AuthMode,
	handler func(RequestContext, TIn) Result[TOut],
) Procedure {
	return FormWithErrorFor[TIn, TOut, TErr](namespace, method).Auth(auth).Handle(handler)
}

func newProcedureBuilder[TIn any, TOut any, TErr any](
	namespace string,
	method string,
	kind ProcedureKind,
) ProcedureBuilder[TIn, TOut, TErr] {
	return ProcedureBuilder[TIn, TOut, TErr]{
		namespace: namespace,
		method:    method,
		kind:      kind,
		auth:      AuthNone,
	}
}

// Auth sets the procedure auth mode.
func (b ProcedureBuilder[TIn, TOut, TErr]) Auth(auth AuthMode) ProcedureBuilder[TIn, TOut, TErr] {
	b.auth = auth
	return b
}

// RequireAuth marks the procedure as requiring an authenticated user.
func (b ProcedureBuilder[TIn, TOut, TErr]) RequireAuth() ProcedureBuilder[TIn, TOut, TErr] {
	return b.Auth(AuthRequired)
}

// OptionalAuth marks the procedure as accepting but not requiring an authenticated user.
func (b ProcedureBuilder[TIn, TOut, TErr]) OptionalAuth() ProcedureBuilder[TIn, TOut, TErr] {
	return b.Auth(AuthOptional)
}

// Doc attaches contract documentation to the procedure.
func (b ProcedureBuilder[TIn, TOut, TErr]) Doc(doc string) ProcedureBuilder[TIn, TOut, TErr] {
	b.doc = doc
	return b
}

// Handle attaches the typed handler and returns the finalized procedure.
func (b ProcedureBuilder[TIn, TOut, TErr]) Handle(handler func(RequestContext, TIn) Result[TOut]) Procedure {
	procedure := newProcedure[TIn, TOut, TErr](b.namespace, b.method, b.kind, b.auth, handler)
	procedure.Contract.Doc = b.doc
	return procedure
}

func newProcedure[TIn any, TOut any, TErr any](
	namespace string,
	method string,
	kind ProcedureKind,
	auth AuthMode,
	handler func(RequestContext, TIn) Result[TOut],
) Procedure {
	var input TIn
	var output TOut
	var errDetail TErr
	inputType := reflect.TypeOf(input)
	outputType := reflect.TypeOf(output)
	errorType := reflect.TypeOf(errDetail)
	if inputType == nil {
		inputType = reflect.TypeOf(Unit{})
	}
	if outputType == nil {
		outputType = reflect.TypeOf(Unit{})
	}
	if errorType == nil {
		errorType = reflect.TypeOf(Unit{})
	}
	validateProcedureTypes(kind, inputType)

	return Procedure{
		Contract: ProcedureContract{
			Name:          namespace + "." + method,
			Namespace:     namespace,
			MethodName:    method,
			ProcedureKind: kind,
			HTTPMethod: func() HTTPMethod {
				if kind == ProcedureQuery {
					return MethodGet
				}
				return MethodPost
			}(),
			Path: "/rpc/" + namespace + "." + method,
			InputEncoding: func() InputEncoding {
				if inputType == reflect.TypeOf(Unit{}) {
					return InputNone
				}
				switch kind {
				case ProcedureQuery:
					return InputQueryString
				case ProcedureCommand:
					return InputJSONBody
				case ProcedureForm:
					return InputFormBody
				default:
					return InputNone
				}
			}(),
			AuthMode:   auth,
			Doc:        "",
			InputType:  exportTypeExpr(inputType),
			OutputType: exportTypeExpr(outputType),
			ErrorType:  exportTypeExpr(errorType),
		},
		InputType:  inputType,
		OutputType: outputType,
		ErrorType:  errorType,
		invoke: func(ctx RequestContext, input any) (bool, any, *AppError) {
			result := handler(ctx, input.(TIn))
			return result.Error == nil, result.Value, result.Error
		},
	}
}

func validateProcedureTypes(kind ProcedureKind, inputType reflect.Type) {
	if inputType == reflect.TypeOf(Unit{}) {
		return
	}

	switch kind {
	case ProcedureQuery, ProcedureForm:
	default:
		return
	}

	if inputType.Kind() == reflect.Pointer {
		panic(fmt.Sprintf("%s inputs must not be pointers, got %s", procedureKindLabel(kind), inputType))
	}

	if inputType.Kind() != reflect.Struct || inputType.Name() == "" {
		panic(fmt.Sprintf("%s inputs must be named struct wrappers, got %s", procedureKindLabel(kind), inputType))
	}
}

func procedureKindLabel(kind ProcedureKind) string {
	switch kind {
	case ProcedureQuery:
		return "query"
	case ProcedureForm:
		return "form"
	default:
		return "procedure"
	}
}
