use serde::Serialize;

use crate::error::{AppError, AppResult};

pub fn print_json<T: Serialize + ?Sized>(value: &T) -> AppResult<()> {
    let payload = serde_json::to_string_pretty(value)
        .map_err(|error| AppError::general(format!("failed to serialize JSON output: {error}")))?;
    println!("{payload}");
    Ok(())
}

pub fn print_json_compact<T: Serialize + ?Sized>(value: &T) -> AppResult<()> {
    let payload = serde_json::to_string(value)
        .map_err(|error| AppError::general(format!("failed to serialize JSON output: {error}")))?;
    println!("{payload}");
    Ok(())
}
