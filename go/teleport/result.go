package teleport

import (
	"encoding/json"
	"net/http"
)

// Unit represents an empty procedure input, output, or error detail.
type Unit struct{}

// AppErrorType is the shared error discriminator serialized to TypeScript.
type AppErrorType string

const (
	// AppErrorUnauthorized maps to HTTP 401.
	AppErrorUnauthorized AppErrorType = "Unauthorized"
	// AppErrorForbidden maps to HTTP 403.
	AppErrorForbidden AppErrorType = "Forbidden"
	// AppErrorNotFound maps to HTTP 404.
	AppErrorNotFound AppErrorType = "NotFound"
	// AppErrorBadRequest maps to HTTP 400 and includes a message.
	AppErrorBadRequest AppErrorType = "BadRequest"
	// AppErrorInternal maps to HTTP 500 and includes a message.
	AppErrorInternal AppErrorType = "Internal"
	// AppErrorRateLimited maps to HTTP 429.
	AppErrorRateLimited AppErrorType = "RateLimited"
	// AppErrorDetail maps to HTTP 422 and carries a typed detail payload.
	AppErrorDetail AppErrorType = "Detail"
)

// AppError is the Go runtime representation of the shared Teleport error union.
type AppError struct {
	Type    AppErrorType
	Message string
	Detail  any
}

// UnauthorizedError returns a 401 Unauthorized error.
func UnauthorizedError() *AppError { return &AppError{Type: AppErrorUnauthorized} }

// ForbiddenError returns a 403 Forbidden error.
func ForbiddenError() *AppError { return &AppError{Type: AppErrorForbidden} }

// NotFoundError returns a 404 Not Found error.
func NotFoundError() *AppError { return &AppError{Type: AppErrorNotFound} }

// BadRequestError returns a 400 Bad Request error with a diagnostic message.
func BadRequestError(message string) *AppError {
	return &AppError{Type: AppErrorBadRequest, Message: message}
}

// InternalError returns a 500 Internal error with a sanitized diagnostic message.
func InternalError(message string) *AppError {
	return &AppError{Type: AppErrorInternal, Message: message}
}

// RateLimitedError returns a 429 Too Many Requests error.
func RateLimitedError() *AppError { return &AppError{Type: AppErrorRateLimited} }

// DetailError returns a typed application error serialized as the Detail variant.
func DetailError(detail any) *AppError {
	return &AppError{Type: AppErrorDetail, Detail: detail}
}

// StatusCode maps the shared Teleport error variant to its HTTP response status.
func (e *AppError) StatusCode() int {
	if e == nil {
		return http.StatusInternalServerError
	}
	switch e.Type {
	case AppErrorUnauthorized:
		return http.StatusUnauthorized
	case AppErrorForbidden:
		return http.StatusForbidden
	case AppErrorNotFound:
		return http.StatusNotFound
	case AppErrorBadRequest:
		return http.StatusBadRequest
	case AppErrorInternal:
		return http.StatusInternalServerError
	case AppErrorRateLimited:
		return http.StatusTooManyRequests
	case AppErrorDetail:
		return http.StatusUnprocessableEntity
	default:
		return http.StatusInternalServerError
	}
}

// MarshalJSON serializes AppError in the contract-compatible tagged-union shape.
func (e *AppError) MarshalJSON() ([]byte, error) {
	payload := map[string]any{"type": e.Type}
	switch e.Type {
	case AppErrorBadRequest, AppErrorInternal:
		payload["message"] = e.Message
	case AppErrorDetail:
		payload["detail"] = e.Detail
	}
	return json.Marshal(payload)
}

// Result is a typed procedure result that is either an output value or AppError.
type Result[T any] struct {
	Value T
	Error *AppError
}

// Ok returns a successful procedure result.
func Ok[T any](value T) Result[T] {
	return Result[T]{Value: value}
}

// Fail returns a failed procedure result.
func Fail[T any](err *AppError) Result[T] {
	return Result[T]{Error: err}
}
