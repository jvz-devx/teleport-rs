use teleport::teleport_type;

#[teleport_type]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar: Option<String>,
}

#[teleport_type]
pub struct Post {
    pub id: String,
    pub title: String,
    pub content: String,
    pub author_id: String,
    pub tags: Vec<String>,
}

#[teleport_type]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[teleport_type]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
}

#[teleport_type]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

#[teleport_type]
pub struct LoginErrorDetail {
    pub invalid_credentials: bool,
}

#[teleport_type]
pub struct GetUserErrorDetail {
    pub user_not_found: bool,
}

#[teleport_type]
pub struct GetUserById {
    pub id: String,
}
