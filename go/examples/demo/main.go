package main

import (
	"encoding/json"
	"flag"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strconv"
	"strings"

	"github.com/jvz-devx/teleport-rs/go/teleport"
	"github.com/jvz-devx/teleport-rs/go/teleporthttp"
)

type User struct {
	ID     string  `json:"id"`
	Name   string  `json:"name"`
	Email  string  `json:"email"`
	Avatar *string `json:"avatar"`
}

type Post struct {
	ID       string   `json:"id"`
	Title    string   `json:"title"`
	Content  string   `json:"content"`
	AuthorID string   `json:"author_id"`
	Tags     []string `json:"tags"`
}

type LoginRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

type CreatePostRequest struct {
	Title   string   `json:"title"`
	Content string   `json:"content"`
	Tags    []string `json:"tags"`
}

type LoginResponse struct {
	Token string `json:"token"`
	User  User   `json:"user"`
}

type LoginErrorDetail struct {
	InvalidCredentials bool `json:"invalid_credentials"`
}

type GetUserErrorDetail struct {
	UserNotFound bool `json:"user_not_found"`
}

type GetPostsByAuthor struct {
	AuthorID string `json:"author_id"`
}

type GetUserById struct {
	ID string `json:"id"`
}

type DemoState struct {
	users    []User
	posts    []Post
	sessions map[string]string
	nextID   int
}

func newState() *DemoState {
	aliceAvatar := "https://i.pravatar.cc/150?u=alice"
	return &DemoState{
		users: []User{
			{ID: "1", Name: "Alice", Email: "alice@example.com", Avatar: &aliceAvatar},
			{ID: "2", Name: "Bob", Email: "bob@example.com"},
		},
		posts: []Post{
			{ID: "1", Title: "Hello World", Content: "This is the first post.", AuthorID: "1", Tags: []string{"intro", "hello"}},
			{ID: "2", Title: "Rust is great", Content: "Here is why I love Rust.", AuthorID: "1", Tags: []string{"rust", "programming"}},
		},
		sessions: map[string]string{
			"demo-token-alice": "1",
			"demo-token-bob":   "2",
		},
		nextID: 3,
	}
}

func (s *DemoState) user(id string) *User {
	for _, user := range s.users {
		if user.ID == id {
			copy := user
			return &copy
		}
	}
	return nil
}

func (s *DemoState) usersList() []User {
	return append([]User{}, s.users...)
}

func (s *DemoState) createPost(authorID string, input CreatePostRequest) Post {
	post := Post{
		ID:       strconv.Itoa(s.nextID),
		Title:    input.Title,
		Content:  input.Content,
		AuthorID: authorID,
		Tags:     append([]string{}, input.Tags...),
	}
	s.nextID++
	s.posts = append(s.posts, post)
	return post
}

func main() {
	exportOnly := flag.Bool("export-only", false, "write teleport.contract.json and exit")
	flag.Parse()

	state := newState()
	namedTypes := teleport.ExportNamedTypes(
		reflectType[User](),
		reflectType[Post](),
		reflectType[LoginRequest](),
		reflectType[CreatePostRequest](),
		reflectType[LoginResponse](),
		reflectType[LoginErrorDetail](),
		reflectType[GetUserErrorDetail](),
		reflectType[GetPostsByAuthor](),
		reflectType[GetUserById](),
	)
	for i := range namedTypes {
		if namedTypes[i].Name == "GetPostsByAuthor" {
			namedTypes[i].Docs = " Query input for `get_posts`.\n\n Query inputs must be struct wrappers — `serde_qs` cannot deserialize\n bare primitive types, so even a single-field input needs its own struct."
		}
	}

	router := teleporthttp.New().
		IncludeManifest(true).
		SetAuthenticator(func(req *http.Request) (any, bool) {
			header := req.Header.Get("Authorization")
			if strings.HasPrefix(strings.ToLower(header), "bearer ") {
				token := strings.TrimSpace(header[7:])
				id, ok := state.sessions[token]
				return id, ok
			}
			return nil, false
		}).
		AddTypes(namedTypes...).
		Register(
			teleport.CommandWithErrorFor[LoginRequest, LoginResponse, LoginErrorDetail]("auth", "login").
				Doc("Authenticate with email and password.").
				Handle(func(ctx teleport.RequestContext, input LoginRequest) teleport.Result[LoginResponse] {
					for _, user := range state.users {
						if user.Email == input.Email {
							token := "demo-token-alice"
							if user.ID == "2" {
								token = "demo-token-bob"
							}
							return teleport.Ok(LoginResponse{Token: token, User: user})
						}
					}
					return teleport.Fail[LoginResponse](teleport.DetailError(LoginErrorDetail{InvalidCredentials: true}))
				}),
			teleport.CommandFor[teleport.Unit, teleport.Unit]("auth", "logout").
				RequireAuth().
				Doc("Log out the current session (no-op in this demo).").
				Handle(func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[teleport.Unit] {
					return teleport.Ok(teleport.Unit{})
				}),
			teleport.QueryFor[teleport.Unit, User]("auth", "getProfile").
				RequireAuth().
				Doc("Get the currently authenticated user's profile.").
				Handle(func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[User] {
					id, _ := ctx.User.(string)
					user := state.user(id)
					if user == nil {
						return teleport.Fail[User](teleport.NotFoundError())
					}
					return teleport.Ok(*user)
				}),
			teleport.QueryWithErrorFor[GetUserById, User, GetUserErrorDetail]("users", "getUser").
				Doc("Fetch a single user by ID.").
				Handle(func(ctx teleport.RequestContext, input GetUserById) teleport.Result[User] {
					user := state.user(input.ID)
					if user == nil {
						return teleport.Fail[User](teleport.DetailError(GetUserErrorDetail{UserNotFound: true}))
					}
					return teleport.Ok(*user)
				}),
			teleport.QueryFor[teleport.Unit, []User]("users", "listUsers").
				Doc("List all registered users.").
				Handle(func(ctx teleport.RequestContext, input teleport.Unit) teleport.Result[[]User] {
					return teleport.Ok(state.usersList())
				}),
			teleport.QueryFor[GetPostsByAuthor, []Post]("posts", "getPosts").
				Doc("Get posts, optionally filtered by author ID.").
				Handle(func(ctx teleport.RequestContext, input GetPostsByAuthor) teleport.Result[[]Post] {
					posts := []Post{}
					for _, post := range state.posts {
						if input.AuthorID == "" || post.AuthorID == input.AuthorID {
							posts = append(posts, post)
						}
					}
					return teleport.Ok(posts)
				}),
			teleport.CommandFor[CreatePostRequest, Post]("posts", "createPost").
				RequireAuth().
				Doc("Create a new post (requires authentication).").
				Handle(func(ctx teleport.RequestContext, input CreatePostRequest) teleport.Result[Post] {
					id, _ := ctx.User.(string)
					return teleport.Ok(state.createPost(id, input))
				}),
		)

	contractPath := filepath.Join(projectDir(), "teleport.contract.json")
	data, err := json.MarshalIndent(router.Contract(), "", "  ")
	if err != nil {
		log.Fatal(err)
	}
	if err := os.WriteFile(contractPath, data, 0o644); err != nil {
		log.Fatal(err)
	}
	log.Printf("Exported contract to %s", contractPath)
	if *exportOnly {
		return
	}

	log.Printf("Server running on http://localhost:3000")
	log.Fatal(http.ListenAndServe("0.0.0.0:3000", router))
}

func projectDir() string {
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		dir, _ := os.Getwd()
		return filepath.Clean(dir)
	}
	return filepath.Dir(file)
}

func reflectType[T any]() reflect.Type {
	var zero T
	return reflect.TypeOf(zero)
}
