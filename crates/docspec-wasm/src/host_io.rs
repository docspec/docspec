use alloc::rc::Rc;
use core::cell::{Cell, RefCell};
use std::io::{self, Read, Seek, SeekFrom, Write};

use wasm_bindgen::{JsCast as _, JsValue};

use crate::error::{exception_message, js_error, ErrorCode};

pub(crate) const MAX_TRANSFER_BYTES: usize = 64 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone)]
struct Callbacks {
    read_at: js_sys::Function,
    source: JsValue,
    write: js_sys::Function,
    /// Receiver for `write`. `JsValue::UNDEFINED` when the host supplied a bare
    /// function, or the sink object when it supplied `{ write(chunk) }`.
    write_this: JsValue,
}

thread_local! {
    /// True only while a host callback is executing.
    ///
    /// This is deliberately narrow. An earlier design latched a flag for the
    /// whole conversion, which a JS exception could strand: wasm-bindgen's
    /// `call1`/`call2` shims catch and return `Err`, but a handful of generated
    /// shims (`__wbg_length_*`, `__wbg_message_*`) do not, and an exception
    /// escaping through a wasm frame skips Rust destructors. Those uncaught
    /// shims all run *outside* the window below, so this flag cannot be
    /// stranded, whereas a conversion-wide flag permanently bricked the
    /// instance and reported every later call as a recursion error.
    static IN_CALLBACK: Cell<bool> = const { Cell::new(false) };
    static CALLBACKS: RefCell<Option<Callbacks>> = const { RefCell::new(None) };
    static FAILURE: RefCell<Option<(ErrorCode, String)>> = const { RefCell::new(None) };
}

/// Marks the dynamic extent of a host callback so re-entry can be rejected.
struct CallbackScope;

impl CallbackScope {
    fn enter() -> Self {
        IN_CALLBACK.with(|flag| flag.set(true));
        Self
    }
}

impl Drop for CallbackScope {
    fn drop(&mut self) {
        IN_CALLBACK.with(|flag| flag.set(false));
    }
}

pub(crate) struct HostIoGuard {
    size: u64,
}

impl HostIoGuard {
    pub(crate) fn register(source: &JsValue, write: &JsValue) -> Result<Self, JsValue> {
        if IN_CALLBACK.with(Cell::get) {
            return Err(js_error(
                ErrorCode::InvalidArgument,
                "convert_stream cannot be called from inside a host callback",
            ));
        }
        Self::register_active(source, write)
    }

    fn register_active(source: &JsValue, write_value: &JsValue) -> Result<Self, JsValue> {
        let (write_callback, write_this) = resolve_sink(write_value)?;
        let size_value =
            js_sys::Reflect::get(source, &JsValue::from_str("size")).map_err(|value| {
                js_error(
                    ErrorCode::InvalidArgument,
                    &format!("failed to read source.size: {}", exception_message(&value)),
                )
            })?;
        if !js_sys::Number::is_safe_integer(&size_value) {
            return Err(js_error(
                ErrorCode::InvalidArgument,
                "source.size must be a nonnegative safe integer",
            ));
        }
        let size_number = size_value.as_f64().ok_or_else(|| {
            js_error(
                ErrorCode::InvalidArgument,
                "source.size must be a nonnegative safe integer",
            )
        })?;
        if size_number < 0.0 {
            return Err(js_error(
                ErrorCode::InvalidArgument,
                "source.size must be a nonnegative safe integer",
            ));
        }
        let read_at = js_sys::Reflect::get(source, &JsValue::from_str("readAt"))
            .map_err(|value| {
                js_error(
                    ErrorCode::InvalidArgument,
                    &format!(
                        "failed to read source.readAt: {}",
                        exception_message(&value)
                    ),
                )
            })?
            .dyn_into::<js_sys::Function>()
            .map_err(|_value| {
                js_error(
                    ErrorCode::InvalidArgument,
                    "source.readAt must be a function",
                )
            })?;
        // Overwriting rather than requiring an empty slot is deliberate: if an
        // earlier conversion ended by an exception escaping through a wasm
        // frame, its guard never ran and its callbacks are still installed.
        // Replacing them here drops that reference, releasing whatever the host
        // retained through it (a File, a Blob, an OPFS access handle).
        CALLBACKS.with(|slot| {
            *slot.borrow_mut() = Some(Callbacks {
                source: source.clone(),
                read_at,
                write: write_callback,
                write_this,
            });
        });
        FAILURE.with(|failure| *failure.borrow_mut() = None);
        Ok(Self {
            size: size_number as u64,
        })
    }

    pub(crate) const fn size(&self) -> u64 {
        self.size
    }
}

impl Drop for HostIoGuard {
    fn drop(&mut self) {
        CALLBACKS.with(|slot| *slot.borrow_mut() = None);
        FAILURE.with(|failure| *failure.borrow_mut() = None);
    }
}

pub(crate) struct HostReader {
    position: u64,
    size: u64,
}

impl HostReader {
    pub(crate) const fn new(size: u64) -> Self {
        Self { position: 0, size }
    }
}

impl Read for HostReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // `>=`, not `==`. Seeking past the end is legal under the Seek contract
        // and reaching there must read as EOF. A ZIP central directory carries
        // attacker-controlled local-header offsets that are never bounded
        // against file length, so `position > size` is reachable from a merely
        // corrupt document. With `==` that fell through and called the host
        // with a zero-length read past the end, which surfaced as a host I/O
        // failure whose code varied by host implementation instead of as a
        // conversion error attributable to the document.
        if buf.is_empty() || self.position >= self.size {
            return Ok(0);
        }
        let remaining = self.size.saturating_sub(self.position);
        let requested = buf
            .len()
            .min(MAX_TRANSFER_BYTES)
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        if requested == 0 {
            return Ok(0);
        }
        let callbacks = callbacks()?;
        let value = {
            let _scope = CallbackScope::enter();
            callbacks.read_at.call2(
                &callbacks.source,
                &JsValue::from_f64(self.position as f64),
                &JsValue::from_f64(requested as f64),
            )
        }
        .map_err(|value| host_failure(ErrorCode::Io, exception_message(&value)))?;
        // Type check first. Testing for a thenable up front rejected a genuine
        // Uint8Array that merely carried a `then` property, and paid two
        // property lookups on every chunk of the happy path.
        let bytes = match value.dyn_into::<js_sys::Uint8Array>() {
            Ok(bytes) => bytes,
            Err(returned) => {
                return Err(if is_promise(&returned) {
                    host_failure(
                        ErrorCode::InvalidArgument,
                        String::from("source.readAt must return synchronously"),
                    )
                } else {
                    host_failure(
                        ErrorCode::InvalidArgument,
                        String::from("source.readAt must return a Uint8Array"),
                    )
                });
            }
        };
        let actual = usize::try_from(bytes.length())
            .map_err(|error| host_failure(ErrorCode::InvalidArgument, error.to_string()))?;
        if actual > requested {
            return Err(host_failure(
                ErrorCode::InvalidArgument,
                String::from("source.readAt returned more bytes than requested"),
            ));
        }
        if actual == 0 {
            // The request was in range and non-zero, so an empty result means
            // the host's `size` and its bytes disagree. That is a host problem.
            return Err(host_failure(
                ErrorCode::Io,
                String::from("source.readAt reached EOF before source.size"),
            ));
        }
        let destination = buf.get_mut(..actual).ok_or_else(|| {
            host_failure(
                ErrorCode::InvalidArgument,
                String::from("source.readAt returned an invalid byte count"),
            )
        })?;
        // `copy_to` re-reads `length` and asserts it still equals the
        // destination length. For a genuine Uint8Array that is stable, so this
        // is sound. A host that returns an exotic object with a `length`
        // accessor returning different values on successive reads would abort
        // the module; that is out of scope, because the host is the embedding
        // application and can already do anything.
        bytes.copy_to(destination);
        self.position = self
            .position
            .checked_add(
                u64::try_from(actual)
                    .map_err(|error| host_failure(ErrorCode::InvalidArgument, error.to_string()))?,
            )
            .ok_or_else(|| reader_error("source position overflowed"))?;
        Ok(actual)
    }
}

impl Seek for HostReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let next = match pos {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.size)
                .checked_add(i128::from(offset))
                .ok_or_else(|| reader_error("seek offset overflowed"))?,
            SeekFrom::Current(offset) => i128::from(self.position)
                .checked_add(i128::from(offset))
                .ok_or_else(|| reader_error("seek offset overflowed"))?,
        };
        if !(0..=i128::from(MAX_SAFE_INTEGER)).contains(&next) {
            return Err(reader_error(
                "seek offset must be a nonnegative safe integer",
            ));
        }
        self.position =
            u64::try_from(next).map_err(|_error| reader_error("seek offset is out of range"))?;
        Ok(self.position)
    }
}

#[derive(Clone)]
pub(crate) struct HostWriter {
    buffer: Rc<RefCell<Vec<u8>>>,
}

impl HostWriter {
    pub(crate) fn finish(mut self) -> io::Result<()> {
        self.flush()
    }

    pub(crate) fn new() -> Self {
        Self {
            buffer: Rc::new(RefCell::new(Vec::with_capacity(MAX_TRANSFER_BYTES))),
        }
    }
}

impl Write for HostWriter {
    fn flush(&mut self) -> io::Result<()> {
        let mut buffer = self.buffer.borrow_mut();
        if buffer.is_empty() {
            return Ok(());
        }
        let callbacks = callbacks()?;
        let chunk = js_sys::Uint8Array::from(buffer.as_slice());
        // The borrow above is held across this call. That is safe only because
        // re-entering convert_stream from inside a host callback is rejected;
        // adding an export that touches HostWriter would break the invariant.
        let result = {
            let _scope = CallbackScope::enter();
            callbacks.write.call1(&callbacks.write_this, chunk.as_ref())
        }
        .map_err(|value| host_failure(ErrorCode::Io, exception_message(&value)))?;
        if is_promise(&result) {
            return Err(host_failure(
                ErrorCode::InvalidArgument,
                String::from("write must return synchronously"),
            ));
        }
        buffer.clear();
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let available = MAX_TRANSFER_BYTES.saturating_sub(self.buffer.borrow().len());
        let accepted = buf.len().min(available);
        let source_chunk = buf.get(..accepted).ok_or_else(|| {
            host_failure(
                ErrorCode::InvalidArgument,
                String::from("output chunk length is invalid"),
            )
        })?;
        self.buffer.borrow_mut().extend_from_slice(source_chunk);
        if self.buffer.borrow().len() == MAX_TRANSFER_BYTES {
            self.flush()?;
        }
        Ok(accepted)
    }
}

/// Accepts either a bare `write(chunk)` function or a `{ write(chunk) }` sink.
///
/// The object form exists so the sink can be spelled the same way as the
/// source. Passing a bare method reference (`sink.write`, or an OPFS handle's
/// `handle.write`) is a natural thing to write and would otherwise be invoked
/// with `this === undefined`, failing inside the host with a `TypeError`.
fn resolve_sink(write_value: &JsValue) -> Result<(js_sys::Function, JsValue), JsValue> {
    if let Some(function) = write_value.dyn_ref::<js_sys::Function>() {
        return Ok((function.clone(), JsValue::UNDEFINED));
    }
    if write_value.is_object() {
        let method = js_sys::Reflect::get(write_value, &JsValue::from_str("write"))
            .map_err(|value| {
                js_error(
                    ErrorCode::InvalidArgument,
                    &format!("failed to read write.write: {}", exception_message(&value)),
                )
            })?
            .dyn_into::<js_sys::Function>()
            .map_err(|_value| {
                js_error(
                    ErrorCode::InvalidArgument,
                    "write must be a function or an object with a write method",
                )
            })?;
        return Ok((method, write_value.clone()));
    }
    Err(js_error(
        ErrorCode::InvalidArgument,
        "write must be a function or an object with a write method",
    ))
}

fn callbacks() -> io::Result<Callbacks> {
    if let Some((_, message)) = FAILURE.with(|failure| failure.borrow().clone()) {
        return Err(io::Error::other(message));
    }
    CALLBACKS.with(|slot| {
        slot.borrow()
            .clone()
            .ok_or_else(|| io::Error::other("host callbacks are unavailable"))
    })
}

/// Records a host contract violation, preserving its code across `docspec_core`.
///
/// Only for failures the *host* is responsible for. An `io::Error` loses its
/// identity on the way back through the pipeline, so the code is latched here
/// and recovered by [`conversion_error`].
fn host_failure(code: ErrorCode, message: String) -> io::Error {
    FAILURE.with(|failure| {
        let mut recorded = failure.borrow_mut();
        if recorded.is_none() {
            *recorded = Some((code, message.clone()));
        }
    });
    io::Error::other(message)
}

/// Reports a request the *document* made that the source cannot satisfy.
///
/// Deliberately does not latch: these must degrade to `CONVERSION_ERROR`,
/// because a reader asking for an out-of-range offset means the document is
/// malformed, not that the host misbehaved. Latching them made a corrupt DOCX
/// indistinguishable from a storage failure.
fn reader_error(message: &str) -> io::Error {
    io::Error::other(String::from(message))
}

pub(crate) fn conversion_error(message: &str) -> JsValue {
    let failure = FAILURE.with(|failure| failure.borrow_mut().take());
    match failure {
        Some((code, host_message)) => js_error(code, &host_message),
        None => js_error(ErrorCode::Conversion, message),
    }
}

pub(crate) fn finish_conversion() -> Result<(), JsValue> {
    let failure = FAILURE.with(|failure| failure.borrow_mut().take());
    match failure {
        Some((code, message)) => Err(js_error(code, &message)),
        None => Ok(()),
    }
}

fn is_promise(value: &JsValue) -> bool {
    if value.is_instance_of::<js_sys::Promise>() {
        return true;
    }
    js_sys::Reflect::get(value, &JsValue::from_str("then")).is_ok_and(|then| then.is_function())
}
