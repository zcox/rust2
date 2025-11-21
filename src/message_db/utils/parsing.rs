/// Extract the entity ID portion from a stream name.
///
/// Returns the ID portion after the first hyphen, or None if no hyphen exists.
/// Works correctly with category types (colons and plus signs remain in category).
///
/// # Examples
///
/// ```
/// use rust2::message_db::utils::parsing::id;
///
/// assert_eq!(id("account-123"), Some("123".to_string()));
/// assert_eq!(id("account-123-456"), Some("123-456".to_string()));
/// assert_eq!(id("account"), None);
/// assert_eq!(id("account:command-123"), Some("123".to_string()));
/// assert_eq!(id("account:v0-streamId"), Some("streamId".to_string()));
/// assert_eq!(id("transaction:event+audit-xyz"), Some("xyz".to_string()));
/// assert_eq!(id("account:command"), None);
/// ```
pub fn id(stream_name: &str) -> Option<String> {
    stream_name
        .find('-')
        .map(|pos| stream_name[pos + 1..].to_string())
}

/// Extract the base entity ID (first segment after category) from a stream name.
///
/// This is useful when IDs contain hyphens and you need just the primary identifier.
/// Returns the cardinal ID, or None if no hyphen exists.
///
/// # Examples
///
/// ```
/// use rust2::message_db::utils::parsing::cardinal_id;
///
/// assert_eq!(cardinal_id("account-123"), Some("123".to_string()));
/// assert_eq!(cardinal_id("account-123-456"), Some("123".to_string()));
/// assert_eq!(cardinal_id("account"), None);
/// assert_eq!(cardinal_id("account:command-123"), Some("123".to_string()));
/// assert_eq!(cardinal_id("account:v0-streamId"), Some("streamId".to_string()));
/// assert_eq!(cardinal_id("withdrawal:position-consumer-1"), Some("consumer".to_string()));
/// assert_eq!(cardinal_id("account:command"), None);
/// ```
pub fn cardinal_id(stream_name: &str) -> Option<String> {
    id(stream_name).map(|id_part| {
        id_part
            .find('-')
            .map(|pos| id_part[..pos].to_string())
            .unwrap_or(id_part)
    })
}

/// Extract the category portion from a stream name, including any type qualifiers.
///
/// Returns everything before the first hyphen (`-`), including all category type
/// qualifiers (colons and plus signs). If no hyphen exists, returns the entire
/// stream name.
///
/// # Examples
///
/// ```
/// use rust2::message_db::utils::parsing::category;
///
/// assert_eq!(category("account-123"), "account");
/// assert_eq!(category("account"), "account");
/// assert_eq!(category("account:command-123"), "account:command");
/// assert_eq!(category("account:v0-streamId"), "account:v0");
/// assert_eq!(category("transaction:event+audit-xyz"), "transaction:event+audit");
/// assert_eq!(category("account:command"), "account:command");
/// assert_eq!(category("withdrawal:position-consumer-1"), "withdrawal:position");
/// ```
pub fn category(stream_name: &str) -> String {
    stream_name
        .find('-')
        .map(|pos| stream_name[..pos].to_string())
        .unwrap_or_else(|| stream_name.to_string())
}

/// Determine if a stream name represents a category (no ID portion).
///
/// Returns true if stream name contains no hyphen (`-`).
/// Category types (colons and plus signs) do not affect the result.
///
/// # Examples
///
/// ```
/// use rust2::message_db::utils::parsing::is_category;
///
/// assert_eq!(is_category("account"), true);
/// assert_eq!(is_category("account-123"), false);
/// assert_eq!(is_category("account:command"), true);
/// assert_eq!(is_category("account:command-123"), false);
/// assert_eq!(is_category("transaction:event+audit"), true);
/// assert_eq!(is_category("transaction:event+audit-xyz"), false);
/// ```
pub fn is_category(stream_name: &str) -> bool {
    !stream_name.contains('-')
}

/// Extract the type qualifiers from a category.
///
/// Returns list of individual type qualifiers, or empty list if none present.
/// This is an optional utility operation.
///
/// # Examples
///
/// ```
/// use rust2::message_db::utils::parsing::get_category_types;
///
/// assert_eq!(get_category_types("account-123"), Vec::<String>::new());
/// assert_eq!(get_category_types("account:command-123"), vec!["command"]);
/// assert_eq!(get_category_types("account:v0-streamId"), vec!["v0"]);
/// assert_eq!(get_category_types("transaction:event+audit-xyz"), vec!["event", "audit"]);
/// assert_eq!(get_category_types("order:snapshot+v2+compressed"), vec!["snapshot", "v2", "compressed"]);
/// assert_eq!(get_category_types("account"), Vec::<String>::new());
/// assert_eq!(get_category_types("account:command"), vec!["command"]);
/// ```
pub fn get_category_types(stream_name: &str) -> Vec<String> {
    let cat = category(stream_name);

    cat.find(':')
        .map(|pos| {
            cat[pos + 1..]
                .split('+')
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_else(Vec::new)
}

/// Extract just the base category name without type qualifiers.
///
/// Returns the base category name without types. This is an optional utility operation.
///
/// # Examples
///
/// ```
/// use rust2::message_db::utils::parsing::get_base_category;
///
/// assert_eq!(get_base_category("account-123"), "account");
/// assert_eq!(get_base_category("account:command-123"), "account");
/// assert_eq!(get_base_category("account:v0-streamId"), "account");
/// assert_eq!(get_base_category("transaction:event+audit-xyz"), "transaction");
/// assert_eq!(get_base_category("account"), "account");
/// assert_eq!(get_base_category("account:command"), "account");
/// ```
pub fn get_base_category(stream_name: &str) -> String {
    let cat = category(stream_name);

    cat.find(':')
        .map(|pos| cat[..pos].to_string())
        .unwrap_or(cat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id() {
        assert_eq!(id("account-123"), Some("123".to_string()));
        assert_eq!(id("account-123-456"), Some("123-456".to_string()));
        assert_eq!(id("account"), None);
        assert_eq!(id("account:command-123"), Some("123".to_string()));
        assert_eq!(id("account:v0-streamId"), Some("streamId".to_string()));
        assert_eq!(
            id("transaction:event+audit-xyz"),
            Some("xyz".to_string())
        );
        assert_eq!(id("account:command"), None);
    }

    #[test]
    fn test_cardinal_id() {
        assert_eq!(cardinal_id("account-123"), Some("123".to_string()));
        assert_eq!(cardinal_id("account-123-456"), Some("123".to_string()));
        assert_eq!(cardinal_id("account"), None);
        assert_eq!(cardinal_id("account:command-123"), Some("123".to_string()));
        assert_eq!(
            cardinal_id("account:v0-streamId"),
            Some("streamId".to_string())
        );
        assert_eq!(
            cardinal_id("withdrawal:position-consumer-1"),
            Some("consumer".to_string())
        );
        assert_eq!(cardinal_id("account:command"), None);
    }

    #[test]
    fn test_category() {
        assert_eq!(category("account-123"), "account");
        assert_eq!(category("account"), "account");
        assert_eq!(category("account:command-123"), "account:command");
        assert_eq!(category("account:v0-streamId"), "account:v0");
        assert_eq!(
            category("transaction:event+audit-xyz"),
            "transaction:event+audit"
        );
        assert_eq!(category("account:command"), "account:command");
        assert_eq!(
            category("withdrawal:position-consumer-1"),
            "withdrawal:position"
        );
    }

    #[test]
    fn test_is_category() {
        assert_eq!(is_category("account"), true);
        assert_eq!(is_category("account-123"), false);
        assert_eq!(is_category("account:command"), true);
        assert_eq!(is_category("account:command-123"), false);
        assert_eq!(is_category("transaction:event+audit"), true);
        assert_eq!(is_category("transaction:event+audit-xyz"), false);
    }

    #[test]
    fn test_get_category_types() {
        assert_eq!(
            get_category_types("account-123"),
            Vec::<String>::new()
        );
        assert_eq!(
            get_category_types("account:command-123"),
            vec!["command".to_string()]
        );
        assert_eq!(
            get_category_types("account:v0-streamId"),
            vec!["v0".to_string()]
        );
        assert_eq!(
            get_category_types("transaction:event+audit-xyz"),
            vec!["event".to_string(), "audit".to_string()]
        );
        assert_eq!(
            get_category_types("order:snapshot+v2+compressed"),
            vec!["snapshot".to_string(), "v2".to_string(), "compressed".to_string()]
        );
        assert_eq!(get_category_types("account"), Vec::<String>::new());
        assert_eq!(
            get_category_types("account:command"),
            vec!["command".to_string()]
        );
    }

    #[test]
    fn test_get_base_category() {
        assert_eq!(get_base_category("account-123"), "account");
        assert_eq!(get_base_category("account:command-123"), "account");
        assert_eq!(get_base_category("account:v0-streamId"), "account");
        assert_eq!(
            get_base_category("transaction:event+audit-xyz"),
            "transaction"
        );
        assert_eq!(get_base_category("account"), "account");
        assert_eq!(get_base_category("account:command"), "account");
    }
}
