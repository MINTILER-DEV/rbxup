use std::fmt::{Display, Formatter};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatorTarget {
    User(String),
    Group(String),
}

impl CreatorTarget {
    pub fn parse(value: &str) -> AppResult<Self> {
        let trimmed = value.trim();
        let (kind, id) = trimmed.split_once(':').ok_or_else(|| {
            AppError::invalid_args(format!(
                "invalid creator `{trimmed}`. Expected user:<id> or group:<id>"
            ))
        })?;

        if id.is_empty() || !id.chars().all(|char| char.is_ascii_digit()) {
            return Err(AppError::invalid_args(format!(
                "invalid creator `{trimmed}`. The id must be all digits"
            )));
        }

        match kind {
            "user" => Ok(Self::User(id.to_string())),
            "group" => Ok(Self::Group(id.to_string())),
            _ => Err(AppError::invalid_args(format!(
                "invalid creator `{trimmed}`. Expected user:<id> or group:<id>"
            ))),
        }
    }

    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::User(id) => Some(id),
            Self::Group(_) => None,
        }
    }

    pub fn group_id(&self) -> Option<&str> {
        match self {
            Self::Group(id) => Some(id),
            Self::User(_) => None,
        }
    }
}

impl Display for CreatorTarget {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User(id) => write!(formatter, "user:{id}"),
            Self::Group(id) => write!(formatter, "group:{id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CreatorTarget;

    #[test]
    fn parses_user_creator() {
        let creator = CreatorTarget::parse("user:12345").expect("creator should parse");
        assert_eq!(creator.to_string(), "user:12345");
        assert_eq!(creator.user_id(), Some("12345"));
        assert_eq!(creator.group_id(), None);
    }

    #[test]
    fn rejects_invalid_creator() {
        let error = CreatorTarget::parse("person:123").expect_err("creator should fail");
        assert_eq!(
            error.to_string(),
            "invalid creator `person:123`. Expected user:<id> or group:<id>"
        );
    }
}
