/// Convert a `snake_case` identifier to `camelCase`.
///
/// # Examples
///
/// ```
/// use teleport_build::naming::snake_to_camel;
///
/// assert_eq!(snake_to_camel("get_user"), "getUser");
/// assert_eq!(snake_to_camel("get_user_profile"), "getUserProfile");
/// assert_eq!(snake_to_camel("id"), "id");
/// assert_eq!(snake_to_camel(""), "");
/// ```
#[must_use]
pub fn snake_to_camel(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;

    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    result
}

/// Convert a `snake_case` identifier to `PascalCase`.
///
/// # Examples
///
/// ```
/// use teleport_build::naming::snake_to_pascal;
///
/// assert_eq!(snake_to_pascal("get_user"), "GetUser");
/// assert_eq!(snake_to_pascal("user"), "User");
/// assert_eq!(snake_to_pascal(""), "");
/// ```
#[must_use]
pub fn snake_to_pascal(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;

    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    result
}

/// Split a qualified procedure name into `(namespace, method_name)`.
///
/// # Examples
///
/// ```
/// use teleport_build::naming::split_namespace;
///
/// assert_eq!(split_namespace("users.getUser"), ("users", "getUser"));
/// assert_eq!(split_namespace("getUser"), ("", "getUser"));
/// ```
#[must_use]
pub fn split_namespace(name: &str) -> (&str, &str) {
    match name.rsplit_once('.') {
        Some((ns, method)) => (ns, method),
        None => ("", name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_word() {
        assert_eq!(snake_to_camel("user"), "user");
    }

    #[test]
    fn two_words() {
        assert_eq!(snake_to_camel("get_user"), "getUser");
    }

    #[test]
    fn three_words() {
        assert_eq!(snake_to_camel("get_user_profile"), "getUserProfile");
    }

    #[test]
    fn empty_string() {
        assert_eq!(snake_to_camel(""), "");
    }

    #[test]
    fn already_camel() {
        assert_eq!(snake_to_camel("getUser"), "getUser");
    }

    #[test]
    fn leading_underscore() {
        assert_eq!(snake_to_camel("_private"), "Private");
    }

    #[test]
    fn consecutive_underscores() {
        assert_eq!(snake_to_camel("get__user"), "getUser");
    }

    #[test]
    fn pascal_single_word() {
        assert_eq!(snake_to_pascal("user"), "User");
    }

    #[test]
    fn pascal_two_words() {
        assert_eq!(snake_to_pascal("get_user"), "GetUser");
    }

    #[test]
    fn pascal_empty() {
        assert_eq!(snake_to_pascal(""), "");
    }

    #[test]
    fn namespace_with_dot() {
        assert_eq!(split_namespace("users.getUser"), ("users", "getUser"));
    }

    #[test]
    fn namespace_without_dot() {
        assert_eq!(split_namespace("getUser"), ("", "getUser"));
    }

    #[test]
    fn namespace_nested() {
        assert_eq!(
            split_namespace("api.users.getUser"),
            ("api.users", "getUser")
        );
    }
}
