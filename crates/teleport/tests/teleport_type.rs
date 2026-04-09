use teleport::teleport_type;

#[teleport_type]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[teleport_type]
pub enum Status {
    Active,
    Inactive,
}

#[test]
fn struct_serializes_and_deserializes() {
    let req = LoginRequest {
        email: "a@b.com".into(),
        password: "secret".into(),
    };
    let json = serde_json::to_string(&req).ok();
    assert!(json.is_some());

    let back: Result<LoginRequest, _> = serde_json::from_str(json.as_deref().unwrap_or("{}"));
    assert!(back.is_ok());
}

#[test]
fn enum_serializes_and_deserializes() {
    let status = Status::Active;
    let json = serde_json::to_string(&status).ok();
    assert!(json.is_some());

    let back: Result<Status, _> = serde_json::from_str(json.as_deref().unwrap_or("\"\""));
    assert!(back.is_ok());
}

#[test]
fn has_specta_type() {
    // Verifies that specta::Type is implemented by checking we can call the type method.
    fn assert_specta_type<T: specta::Type>() {}
    assert_specta_type::<LoginRequest>();
    assert_specta_type::<Status>();
}
