pub mod fallback;

use crate::WorkRequest;

pub fn validate_work_request(req: &WorkRequest) -> Result<(), String> {
    if req.request_id.trim().is_empty() {
        return Err("request_id is required".into());
    }
    if req.prompt_context.trim().is_empty() {
        return Err("prompt_context is required".into());
    }
    if req.prompt_context.len() > 16_000 {
        return Err("prompt_context exceeds 16k characters".into());
    }
    if !(1..=1024).contains(&req.min_tokens) {
        return Err("min_tokens must be in range 1..=1024".into());
    }
    Ok(())
}
