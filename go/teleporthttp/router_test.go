package teleporthttp_test

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"

	"github.com/jvz-devx/teleport-rs/go/teleport"
	"github.com/jvz-devx/teleport-rs/go/teleporthttp"
)

type filterInput struct {
	AuthorID      int  `json:"author_id"`
	IncludeHidden bool `json:"include_hidden"`
}

type searchRequest struct {
	Filter filterInput `json:"filter"`
	Tags   []string    `json:"tags"`
}

type optionalSearch struct {
	Q    *string `json:"q"`
	Page *int    `json:"page"`
}

type createThing struct {
	Title string `json:"title"`
	Count int    `json:"count"`
}

type idInput struct {
	ID string `json:"id"`
}

func TestRouterQueryAndFormDecodingParity(t *testing.T) {
	router := newTestRouter(true)

	t.Run("nested query object with append arrays", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(
			http.MethodGet,
			"/rpc/test.search?filter[author_id]=1&filter[include_hidden]=true&tags[]=rust&tags[]=rpc",
			nil,
		))

		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `"1|true|rust,rpc"`)
	})

	t.Run("nested query object with indexed arrays", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(
			http.MethodGet,
			"/rpc/test.search?filter[author_id]=1&filter[include_hidden]=true&tags[0]=rust&tags[1]=rpc",
			nil,
		))

		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `"1|true|rust,rpc"`)
	})

	t.Run("repeated flat keys still bind slice fields", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(
			http.MethodGet,
			"/rpc/test.search?filter[author_id]=1&filter[include_hidden]=true&tags=rust&tags=rpc",
			nil,
		))

		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `"1|true|rust,rpc"`)
	})

	t.Run("optional fields missing remain nil", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.searchOptional", nil))
		assertStatus(t, rec, http.StatusOK)

		var payload optionalSearch
		decodeJSON(t, rec, &payload)
		if payload.Q != nil || payload.Page != nil {
			t.Fatalf("expected nil optional fields, got %#v", payload)
		}
	})

	t.Run("optional fields present are parsed", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.searchOptional?q=hello&page=2", nil))
		assertStatus(t, rec, http.StatusOK)

		var payload optionalSearch
		decodeJSON(t, rec, &payload)
		if payload.Q == nil || *payload.Q != "hello" {
			t.Fatalf("unexpected q %#v", payload.Q)
		}
		if payload.Page == nil || *payload.Page != 2 {
			t.Fatalf("unexpected page %#v", payload.Page)
		}
	})

	t.Run("form endpoint accepts nested json", func(t *testing.T) {
		body := bytes.NewBufferString(`{"filter":{"author_id":1,"include_hidden":true},"tags":["rust","rpc"]}`)
		req := httptest.NewRequest(http.MethodPost, "/rpc/test.submitSearch", body)
		req.Header.Set("Content-Type", "application/json")

		rec := performRequest(router, req)
		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `"1|true|rust,rpc"`)
	})

	t.Run("form endpoint accepts nested urlencoded payloads", func(t *testing.T) {
		body := "filter[author_id]=1&filter[include_hidden]=true&tags[]=rust&tags[]=rpc"
		req := httptest.NewRequest(http.MethodPost, "/rpc/test.submitSearch", strings.NewReader(body))
		req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

		rec := performRequest(router, req)
		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `"1|true|rust,rpc"`)
	})
}

func TestRouterAuthAndStatusParity(t *testing.T) {
	router := newTestRouter(true)

	t.Run("required auth returns 401 without token and succeeds with token", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.secret", nil))
		assertErrorType(t, rec, http.StatusUnauthorized, "Unauthorized")

		req := httptest.NewRequest(http.MethodGet, "/rpc/test.secret", nil)
		req.Header.Set("Authorization", "Bearer demo-token")
		rec = performRequest(router, req)
		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `"demo-user"`)
	})

	t.Run("optional auth supports anonymous and authenticated requests", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.whoAmI", nil))
		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `"anonymous"`)

		req := httptest.NewRequest(http.MethodGet, "/rpc/test.whoAmI", nil)
		req.Header.Set("Authorization", "Bearer demo-token")
		rec = performRequest(router, req)
		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `"demo-user"`)
	})

	t.Run("explicit 403 404 and 429 mappings are preserved", func(t *testing.T) {
		assertErrorType(t, performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.failForbidden", nil)), http.StatusForbidden, "Forbidden")
		assertErrorType(t, performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.failNotFound", nil)), http.StatusNotFound, "NotFound")
		assertErrorType(t, performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.failRateLimited", nil)), http.StatusTooManyRequests, "RateLimited")
	})
}

func TestRouterBadPayloadMethodManifestAndPanicParity(t *testing.T) {
	router := newTestRouter(true)

	t.Run("bad json returns structured 400", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodPost, "/rpc/test.create", strings.NewReader(`{"count":"oops"}`))
		req.Header.Set("Content-Type", "application/json")

		rec := performRequest(router, req)
		assertErrorType(t, rec, http.StatusBadRequest, "BadRequest")
	})

	t.Run("bad query bool returns structured 400", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(
			http.MethodGet,
			"/rpc/test.search?filter[author_id]=1&filter[include_hidden]=not-a-bool",
			nil,
		))

		assertErrorType(t, rec, http.StatusBadRequest, "BadRequest")
	})

	t.Run("garbage query string returns structured 400", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(
			http.MethodGet,
			"/rpc/test.asyncEcho?id[[invalid",
			nil,
		))

		assertErrorType(t, rec, http.StatusBadRequest, "BadRequest")
	})

	t.Run("missing required query field returns structured 400", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(
			http.MethodGet,
			"/rpc/test.asyncEcho",
			nil,
		))

		assertErrorType(t, rec, http.StatusBadRequest, "BadRequest")
	})

	t.Run("wrong method returns 405", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.create", nil))
		assertStatus(t, rec, http.StatusMethodNotAllowed)
	})

	t.Run("manifest lists registered procedures", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/__manifest", nil))
		assertStatus(t, rec, http.StatusOK)

		var payload map[string]map[string]map[string]string
		decodeJSON(t, rec, &payload)
		entry := payload["procedures"]["test.secret"]
		if entry["method"] != "GET" || entry["path"] != "/rpc/test.secret" {
			t.Fatalf("unexpected manifest entry %#v", entry)
		}
	})

	t.Run("manifest disabled returns 404", func(t *testing.T) {
		rec := performRequest(newTestRouter(false), httptest.NewRequest(http.MethodGet, "/rpc/__manifest", nil))
		assertStatus(t, rec, http.StatusNotFound)
	})

	t.Run("query success and unit null serialization still work", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.asyncEcho?id=abc", nil))
		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `"abc"`)

		rec = performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.ping", nil))
		assertStatus(t, rec, http.StatusOK)
		assertBody(t, rec, `null`)
	})

	t.Run("panic is translated into internal error without leaking payload", func(t *testing.T) {
		rec := performRequest(router, httptest.NewRequest(http.MethodGet, "/rpc/test.crash", nil))
		assertErrorType(t, rec, http.StatusInternalServerError, "Internal")
		if strings.Contains(rec.Body.String(), "boom") {
			t.Fatalf("panic payload leaked into response body: %s", rec.Body.String())
		}
	})
}

func newTestRouter(includeManifest bool) *teleporthttp.Router {
	return teleporthttp.New().
		IncludeManifest(includeManifest).
		Register(
			teleport.Query[searchRequest, string]("test", "search", teleport.AuthNone, func(ctx teleport.RequestContext, input searchRequest) teleport.Result[string] {
				return teleport.Ok(formatSearch(input))
			}),
			teleport.Query[optionalSearch, optionalSearch]("test", "searchOptional", teleport.AuthNone, func(ctx teleport.RequestContext, input optionalSearch) teleport.Result[optionalSearch] {
				return teleport.Ok(input)
			}),
			teleport.Command[createThing, createThing]("test", "create", teleport.AuthNone, func(ctx teleport.RequestContext, input createThing) teleport.Result[createThing] {
				return teleport.Ok(input)
			}),
			teleport.Form[searchRequest, string]("test", "submitSearch", teleport.AuthNone, func(ctx teleport.RequestContext, input searchRequest) teleport.Result[string] {
				return teleport.Ok(formatSearch(input))
			}),
			teleport.Query[teleport.Unit, string]("test", "secret", teleport.AuthRequired, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[string] {
				user, _ := ctx.User.(string)
				return teleport.Ok(user)
			}),
			teleport.Query[teleport.Unit, string]("test", "whoAmI", teleport.AuthOptional, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[string] {
				user, _ := ctx.User.(string)
				if user == "" {
					user = "anonymous"
				}
				return teleport.Ok(user)
			}),
			teleport.Query[teleport.Unit, string]("test", "failForbidden", teleport.AuthNone, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[string] {
				return teleport.Fail[string](teleport.ForbiddenError())
			}),
			teleport.Query[teleport.Unit, string]("test", "failNotFound", teleport.AuthNone, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[string] {
				return teleport.Fail[string](teleport.NotFoundError())
			}),
			teleport.Query[teleport.Unit, string]("test", "failRateLimited", teleport.AuthNone, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[string] {
				return teleport.Fail[string](teleport.RateLimitedError())
			}),
			teleport.Query[idInput, string]("test", "asyncEcho", teleport.AuthNone, func(ctx teleport.RequestContext, input idInput) teleport.Result[string] {
				return teleport.Ok(input.ID)
			}),
			teleport.Query[teleport.Unit, teleport.Unit]("test", "ping", teleport.AuthNone, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[teleport.Unit] {
				return teleport.Ok(teleport.Unit{})
			}),
			teleport.Query[teleport.Unit, teleport.Unit]("test", "crash", teleport.AuthNone, func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[teleport.Unit] {
				panic("boom")
			}),
		).
		SetAuthenticator(func(req *http.Request) (any, bool) {
			if req.Header.Get("Authorization") == "Bearer demo-token" {
				return "demo-user", true
			}
			return nil, false
		})
}

func formatSearch(input searchRequest) string {
	return strings.Join([]string{
		strconv.Itoa(input.Filter.AuthorID),
		strconv.FormatBool(input.Filter.IncludeHidden),
		strings.Join(input.Tags, ","),
	}, "|")
}

func performRequest(router http.Handler, req *http.Request) *httptest.ResponseRecorder {
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	return rec
}

func assertStatus(t *testing.T, rec *httptest.ResponseRecorder, want int) {
	t.Helper()
	if rec.Code != want {
		t.Fatalf("unexpected status %d, body=%s", rec.Code, rec.Body.String())
	}
}

func assertBody(t *testing.T, rec *httptest.ResponseRecorder, want string) {
	t.Helper()
	if got := strings.TrimSpace(rec.Body.String()); got != want {
		t.Fatalf("unexpected body %s", got)
	}
}

func assertErrorType(t *testing.T, rec *httptest.ResponseRecorder, wantStatus int, wantType string) {
	t.Helper()
	assertStatus(t, rec, wantStatus)

	var payload map[string]any
	decodeJSON(t, rec, &payload)
	if got, _ := payload["type"].(string); got != wantType {
		t.Fatalf("unexpected error type %v", payload["type"])
	}
}

func decodeJSON(t *testing.T, rec *httptest.ResponseRecorder, target any) {
	t.Helper()
	if err := json.Unmarshal(rec.Body.Bytes(), target); err != nil {
		t.Fatalf("decode json: %v", err)
	}
}
