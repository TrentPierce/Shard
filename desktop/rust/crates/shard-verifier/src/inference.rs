use anyhow::{Context, Result};
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_ulonglong, c_void, CString};
use std::path::Path;

#[repr(C)]
pub struct ShardInitConfig {
    pub model_path: *const c_char,
    pub layer_start: c_int,
    pub layer_end: c_int,
    pub model_id: *const c_char,
    pub pipeline_mode: c_int,
}

#[repr(C)]
pub struct ShardTensorView {
    pub data: *mut c_void,
    pub byte_size: c_ulonglong,
    pub n_dims: c_int,
    pub dims: [c_ulonglong; 4],
    pub dtype: c_int,
}

#[derive(Debug, Clone)]
pub struct ModelCompatibility {
    pub model_id: String,
    pub protocol_version: String,
    pub supports_speculative: bool,
}

pub fn check_model_compatibility(model_id: &str) -> ModelCompatibility {
    let supports_speculative = !model_id.trim().is_empty();
    ModelCompatibility {
        model_id: model_id.to_string(),
        protocol_version: "v1".to_string(),
        supports_speculative,
    }
}

pub trait VerifierModel {
    fn eval(&mut self, tokens: &[i32]) -> Result<i32>;
    fn tokenize(&mut self, text: &str, max_tokens: usize) -> Result<Vec<i32>>;
    fn get_logits(&mut self, vocab_size: usize) -> Result<Vec<f32>>;
    fn token_to_piece(&mut self, token_id: i32) -> Result<String>;
    fn rollback(&mut self, steps: i32) -> Result<i32>;
}

pub struct ShardEngine {
    _lib: Library,
    handle: *mut c_void,

    // Function pointers must outlive _lib, which they do because struct drop order
    eval_fn: Symbol<'static, unsafe extern "C" fn(*mut c_void, *const c_int, c_int) -> c_int>,
    get_logits_fn: Symbol<'static, unsafe extern "C" fn(*mut c_void, *mut f32, c_int) -> c_int>,
    rollback_fn: Symbol<'static, unsafe extern "C" fn(*mut c_void, c_int) -> c_int>,
    tokenize_fn: Symbol<
        'static,
        unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_int, c_int) -> c_int,
    >,
    token_to_piece_fn:
        Symbol<'static, unsafe extern "C" fn(*mut c_void, c_int, *mut c_char, c_int) -> c_int>,
    free_fn: Symbol<'static, unsafe extern "C" fn(*mut c_void)>,
}

// Safety: The C API uses internal locking or is thread-unsafe by design.
// Assuming it's safe to send between threads but maybe not Sync.
unsafe impl Send for ShardEngine {}

impl ShardEngine {
    #[allow(clippy::missing_transmute_annotations)]
    pub fn load<P: AsRef<Path>>(dll_path: P, config: &ShardInitConfig) -> Result<Self> {
        let path_str = dll_path.as_ref().to_str().unwrap_or_default();
        if path_str.is_empty() {
            anyhow::bail!("Invalid library path");
        }

        unsafe {
            let lib = Library::new(path_str).context("Failed to load shard_engine library")?;

            // Load initialization function
            let init_fn: Symbol<unsafe extern "C" fn(*const ShardInitConfig) -> *mut c_void> =
                lib.get(b"shard_init_ex").context("Missing shard_init_ex")?;

            let handle = init_fn(config);
            if handle.is_null() {
                anyhow::bail!("shard_init_ex returned null handle");
            }

            // Extend lifetimes to static since they are bound to `_lib`
            // and `_lib` is dropped last in the struct.
            let eval_fn = std::mem::transmute(lib.get::<unsafe extern "C" fn(
                *mut c_void,
                *const c_int,
                c_int,
            ) -> c_int>(b"shard_eval")?);
            let get_logits_fn =
                std::mem::transmute(lib.get::<unsafe extern "C" fn(
                    *mut c_void,
                    *mut f32,
                    c_int,
                ) -> c_int>(b"shard_get_logits")?);
            let rollback_fn = std::mem::transmute(
                lib.get::<unsafe extern "C" fn(*mut c_void, c_int) -> c_int>(b"shard_rollback")?,
            );
            let tokenize_fn = std::mem::transmute(lib.get::<unsafe extern "C" fn(
                *mut c_void,
                *const c_char,
                *mut c_int,
                c_int,
            ) -> c_int>(b"shard_tokenize")?);
            let token_to_piece_fn =
                std::mem::transmute(lib.get::<unsafe extern "C" fn(
                    *mut c_void,
                    c_int,
                    *mut c_char,
                    c_int,
                ) -> c_int>(b"shard_token_to_piece")?);
            let free_fn =
                std::mem::transmute(lib.get::<unsafe extern "C" fn(*mut c_void)>(b"shard_free")?);

            Ok(Self {
                _lib: lib,
                handle,
                eval_fn,
                get_logits_fn,
                rollback_fn,
                tokenize_fn,
                token_to_piece_fn,
                free_fn,
            })
        }
    }

    pub fn eval(&self, tokens: &[i32]) -> Result<i32> {
        unsafe {
            let pos = (self.eval_fn)(self.handle, tokens.as_ptr(), tokens.len() as c_int);
            if pos < 0 {
                anyhow::bail!("shard_eval failed with {}", pos);
            }
            Ok(pos)
        }
    }

    pub fn tokenize(&self, text: &str, max_tokens: usize) -> Result<Vec<i32>> {
        let c_text = CString::new(text)?;
        let mut tokens = vec![0i32; max_tokens];
        unsafe {
            let count = (self.tokenize_fn)(
                self.handle,
                c_text.as_ptr(),
                tokens.as_mut_ptr(),
                max_tokens as c_int,
            );
            if count < 0 {
                anyhow::bail!("Tokenization failed");
            }
            tokens.truncate(count as usize);
            Ok(tokens)
        }
    }

    pub fn get_logits(&self, vocab_size: usize) -> Result<Vec<f32>> {
        let mut logits = vec![0.0f32; vocab_size];
        unsafe {
            let read = (self.get_logits_fn)(self.handle, logits.as_mut_ptr(), vocab_size as c_int);
            if read <= 0 {
                anyhow::bail!("get_logits failed or empty");
            }
            logits.truncate(read as usize);
            Ok(logits)
        }
    }

    pub fn token_to_piece(&self, token_id: i32) -> Result<String> {
        let mut buf = vec![0u8; 128];
        unsafe {
            let n = (self.token_to_piece_fn)(
                self.handle,
                token_id,
                buf.as_mut_ptr() as *mut c_char,
                128,
            );
            if n > 0 {
                buf.truncate(n as usize);
                Ok(String::from_utf8_lossy(&buf).into_owned())
            } else {
                Ok(format!("tok_{}", token_id))
            }
        }
    }

    pub fn rollback(&self, steps: i32) -> Result<i32> {
        unsafe {
            let result = (self.rollback_fn)(self.handle, steps);
            if result < 0 {
                anyhow::bail!("rollback failed");
            }
            Ok(result)
        }
    }
}

impl Drop for ShardEngine {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                (self.free_fn)(self.handle);
            }
        }
    }
}

impl VerifierModel for ShardEngine {
    fn eval(&mut self, tokens: &[i32]) -> Result<i32> {
        ShardEngine::eval(self, tokens)
    }

    fn tokenize(&mut self, text: &str, max_tokens: usize) -> Result<Vec<i32>> {
        ShardEngine::tokenize(self, text, max_tokens)
    }

    fn get_logits(&mut self, vocab_size: usize) -> Result<Vec<f32>> {
        ShardEngine::get_logits(self, vocab_size)
    }

    fn token_to_piece(&mut self, token_id: i32) -> Result<String> {
        ShardEngine::token_to_piece(self, token_id)
    }

    fn rollback(&mut self, steps: i32) -> Result<i32> {
        ShardEngine::rollback(self, steps)
    }
}

#[cfg(test)]
mod tests {
    use super::check_model_compatibility;

    #[test]
    fn compatibility_reports_protocol_defaults() {
        let compat = check_model_compatibility("bitnet-1.58b");
        assert_eq!(compat.protocol_version, "v1");
        assert!(compat.supports_speculative);
    }
}
