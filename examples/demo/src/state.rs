use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use teleport::AuthedUser;

use crate::types::{Post, User};

/// Mock application state with in-memory data stores.
///
/// Uses `Arc<Mutex<_>>` for mutable fields so the state can be cloned
/// (required by `TeleportRouter`'s `S: Clone` bound).
#[derive(Debug, Clone)]
pub struct AppState {
    users: Vec<User>,
    posts: Arc<Mutex<Vec<Post>>>,
    sessions: HashMap<String, AuthedUser>,
    next_post_id: Arc<Mutex<u32>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create a new `AppState` with seed data.
    #[must_use]
    pub fn new() -> Self {
        let users = vec![
            User {
                id: "1".into(),
                name: "Alice".into(),
                email: "alice@example.com".into(),
                avatar: Some("https://i.pravatar.cc/150?u=alice".into()),
            },
            User {
                id: "2".into(),
                name: "Bob".into(),
                email: "bob@example.com".into(),
                avatar: None,
            },
        ];

        let posts = vec![
            Post {
                id: "1".into(),
                title: "Hello World".into(),
                content: "This is the first post.".into(),
                author_id: "1".into(),
                tags: vec!["intro".into(), "hello".into()],
            },
            Post {
                id: "2".into(),
                title: "Rust is great".into(),
                content: "Here is why I love Rust.".into(),
                author_id: "1".into(),
                tags: vec!["rust".into(), "programming".into()],
            },
        ];

        let mut sessions = HashMap::new();
        sessions.insert(
            "demo-token-alice".into(),
            AuthedUser {
                id: "1".into(),
                email: "alice@example.com".into(),
            },
        );
        sessions.insert(
            "demo-token-bob".into(),
            AuthedUser {
                id: "2".into(),
                email: "bob@example.com".into(),
            },
        );

        Self {
            users,
            posts: Arc::new(Mutex::new(posts)),
            sessions,
            next_post_id: Arc::new(Mutex::new(3)),
        }
    }

    /// Validate a session token and return the authenticated user, if valid.
    #[must_use]
    pub fn validate_session(&self, token: &str) -> Option<AuthedUser> {
        self.sessions.get(token).cloned()
    }

    /// Find a user by ID.
    #[must_use]
    pub fn get_user(&self, id: &str) -> Option<&User> {
        self.users.iter().find(|u| u.id == id)
    }

    /// Return all users.
    #[must_use]
    pub fn list_users(&self) -> &[User] {
        &self.users
    }

    /// Attempt login with email/password. Returns a session token and user on success.
    #[must_use]
    pub fn login(&self, email: &str, _password: &str) -> Option<(&str, &User)> {
        // In a real app you'd verify the password. Here we accept any password
        // as long as the email matches a known user.
        let user = self.users.iter().find(|u| u.email == email)?;
        let token = self
            .sessions
            .iter()
            .find(|(_, authed)| authed.id == user.id)
            .map(|(token, _)| token.as_str())?;
        Some((token, user))
    }

    /// Get all posts, optionally filtered by author.
    #[must_use]
    pub fn get_posts(&self, author_id: Option<&str>) -> Vec<Post> {
        let posts = self.posts.lock().unwrap_or_else(PoisonError::into_inner);
        author_id.map_or_else(
            || posts.clone(),
            |id| posts.iter().filter(|p| p.author_id == id).cloned().collect(),
        )
    }

    /// Create a new post and return it.
    pub fn create_post(&self, author_id: &str, title: String, content: String, tags: Vec<String>) -> Post {
        let id = {
            let mut next_id = self.next_post_id.lock().unwrap_or_else(PoisonError::into_inner);
            let id = next_id.to_string();
            *next_id += 1;
            id
        };

        let post = Post {
            id,
            title,
            content,
            author_id: author_id.into(),
            tags,
        };

        let mut posts = self.posts.lock().unwrap_or_else(PoisonError::into_inner);
        posts.push(post.clone());
        post
    }
}
