using Teleport.Net;

namespace Teleport.Net.Demo;

public sealed record User(string id, string name, string email, string? avatar);

public sealed record Post(string id, string title, string content, string author_id, string[] tags);

public sealed record LoginRequest(string email, string password);

public sealed record CreatePostRequest(string title, string content, string[] tags);

public sealed record LoginResponse(string token, User user);

public sealed record LoginErrorDetail(bool invalid_credentials);

public sealed record GetUserErrorDetail(bool user_not_found);

[TeleportDoc(" Query input for `get_posts`.\n\n Query inputs must be struct wrappers — `serde_qs` cannot deserialize\n bare primitive types, so even a single-field input needs its own struct.")]
public sealed record GetPostsByAuthor(string author_id);

public sealed record GetUserById(string id);

public sealed class DemoState
{
    private readonly object _gate = new();
    private readonly List<User> _users;
    private readonly List<Post> _posts;
    private readonly Dictionary<string, string> _sessions;
    private int _nextPostId;

    private DemoState()
    {
        _users =
        [
            new User("1", "Alice", "alice@example.com", "https://i.pravatar.cc/150?u=alice"),
            new User("2", "Bob", "bob@example.com", null),
        ];

        _posts =
        [
            new Post("1", "Hello World", "This is the first post.", "1", ["intro", "hello"]),
            new Post("2", "Rust is great", "Here is why I love Rust.", "1", ["rust", "programming"]),
        ];

        _sessions = new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["demo-token-alice"] = "1",
            ["demo-token-bob"] = "2",
        };

        _nextPostId = 3;
    }

    public static DemoState Create() => new();

    public bool TryLogin(string email, string _password, out string token, out User user)
    {
        lock (_gate)
        {
            user = null!;
            var match = _users.FirstOrDefault(user => string.Equals(user.email, email, StringComparison.Ordinal));
            if (match is null)
            {
                token = string.Empty;
                user = null!;
                return false;
            }

            var session = _sessions.FirstOrDefault(entry => entry.Value == match.id);
            if (session.Key is null)
            {
                token = string.Empty;
                user = null!;
                return false;
            }

            token = session.Key;
            user = match;
            return true;
        }
    }

    public bool TryGetUserBySession(string token, out User user)
    {
        lock (_gate)
        {
            user = null!;
            if (!_sessions.TryGetValue(token, out var userId))
            {
                return false;
            }

            var match = _users.FirstOrDefault(user => user.id == userId);
            if (match is null)
            {
                return false;
            }

            user = match;
            return true;
        }
    }

    public User? GetUser(string id)
    {
        lock (_gate)
        {
            return _users.FirstOrDefault(user => user.id == id);
        }
    }

    public User[] ListUsers()
    {
        lock (_gate)
        {
            return _users.ToArray();
        }
    }

    public Post[] GetPosts(string? authorId)
    {
        lock (_gate)
        {
            return authorId is null
                ? _posts.ToArray()
                : _posts.Where(post => post.author_id == authorId).ToArray();
        }
    }

    public Post CreatePost(string authorId, string title, string content, string[] tags)
    {
        lock (_gate)
        {
            var post = new Post(_nextPostId.ToString(), title, content, authorId, tags);
            _nextPostId += 1;
            _posts.Add(post);
            return post;
        }
    }
}

internal static class DemoPaths
{
    public static string ProjectDirectory => Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "..", ".."));

    public static string ContractPath => Path.Combine(ProjectDirectory, "teleport.contract.json");
}
