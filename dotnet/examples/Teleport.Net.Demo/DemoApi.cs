using System.Security.Claims;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Teleport.Net;

namespace Teleport.Net.Demo;

[TeleportModule("auth")]
public static class AuthApi
{
    [TeleportCommand]
    [TeleportDoc("Authenticate with email and password.")]
    public static TeleportResult<LoginResponse, LoginErrorDetail> Login(
        [FromServices] DemoState state,
        LoginRequest input)
    {
        if (!state.TryLogin(input.email, input.password, out var token, out var user))
        {
            return TeleportResult<LoginResponse, LoginErrorDetail>.Fail(
                AppError<LoginErrorDetail>.DetailError(new LoginErrorDetail(true)));
        }

        return TeleportResult<LoginResponse, LoginErrorDetail>.Ok(new LoginResponse(token, user));
    }

    [TeleportCommand]
    [TeleportDoc("Log out the current session (no-op in this demo).")]
    public static TeleportResult<ValueTuple, ValueTuple> Logout([TeleportAuth] ClaimsPrincipal _auth)
    {
        return TeleportResult<ValueTuple, ValueTuple>.Ok(default);
    }

    [TeleportQuery]
    [TeleportName("getProfile")]
    [TeleportDoc("Get the currently authenticated user's profile.")]
    public static TeleportResult<User, ValueTuple> GetProfile(
        [FromServices] DemoState state,
        [TeleportAuth] ClaimsPrincipal auth)
    {
        var user = state.GetUser(DemoAuth.RequireUserId(auth));
        return user is null
            ? TeleportResult<User, ValueTuple>.Fail(AppError<ValueTuple>.NotFoundError())
            : TeleportResult<User, ValueTuple>.Ok(user);
    }
}

[TeleportModule("users")]
public static class UsersApi
{
    [TeleportQuery]
    [TeleportDoc("Fetch a single user by ID.")]
    public static TeleportResult<User, GetUserErrorDetail> GetUser(
        [FromServices] DemoState state,
        GetUserById input)
    {
        var user = state.GetUser(input.id);
        return user is null
            ? TeleportResult<User, GetUserErrorDetail>.Fail(
                AppError<GetUserErrorDetail>.DetailError(new GetUserErrorDetail(true)))
            : TeleportResult<User, GetUserErrorDetail>.Ok(user);
    }

    [TeleportQuery]
    [TeleportDoc("List all registered users.")]
    public static TeleportResult<User[], ValueTuple> ListUsers([FromServices] DemoState state) =>
        TeleportResult<User[], ValueTuple>.Ok(state.ListUsers());
}

[TeleportModule("posts")]
public static class PostsApi
{
    [TeleportQuery]
    [TeleportDoc("Get posts, optionally filtered by author ID.")]
    public static TeleportResult<Post[], ValueTuple> GetPosts(
        [FromServices] DemoState state,
        GetPostsByAuthor input) =>
        TeleportResult<Post[], ValueTuple>.Ok(state.GetPosts(input.author_id));

    [TeleportCommand]
    [TeleportDoc("Create a new post (requires authentication).")]
    public static TeleportResult<Post, ValueTuple> CreatePost(
        [FromServices] DemoState state,
        [TeleportAuth] ClaimsPrincipal auth,
        CreatePostRequest input)
    {
        return TeleportResult<Post, ValueTuple>.Ok(
            state.CreatePost(DemoAuth.RequireUserId(auth), input.title, input.content, input.tags));
    }
}

internal static class DemoAuth
{
    public static bool TryAuthenticate(HttpContext context, DemoState state, out ClaimsPrincipal principal)
    {
        string? sessionToken = null;

        if (context.Request.Cookies.TryGetValue("session", out var cookieToken))
        {
            sessionToken = cookieToken;
        }
        else if (context.Request.Headers.TryGetValue("Authorization", out var authorization))
        {
            var header = authorization.ToString();
            const string prefix = "Bearer ";
            if (header.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
            {
                sessionToken = header[prefix.Length..].Trim();
            }
        }

        if (string.IsNullOrWhiteSpace(sessionToken) || !state.TryGetUserBySession(sessionToken, out var user))
        {
            principal = new ClaimsPrincipal(new ClaimsIdentity());
            return false;
        }

        principal = CreatePrincipal(user);
        return true;
    }

    public static ClaimsPrincipal CreatePrincipal(User user)
    {
        var identity = new ClaimsIdentity(
        [
            new Claim(ClaimTypes.NameIdentifier, user.id),
            new Claim(ClaimTypes.Email, user.email),
            new Claim(ClaimTypes.Name, user.name),
        ], "demo-session");
        return new ClaimsPrincipal(identity);
    }

    public static string RequireUserId(ClaimsPrincipal principal)
    {
        var userId = principal.FindFirstValue(ClaimTypes.NameIdentifier);
        if (string.IsNullOrWhiteSpace(userId))
        {
            throw new InvalidOperationException("authenticated principal is missing a name identifier claim");
        }

        return userId;
    }
}
