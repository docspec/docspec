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
}

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static CALLBACKS: RefCell<Option<Callbacks>> = const { RefCell::new(None) };
    static FAILURE: RefCell<Option<(ErrorCode, String)>> = const { RefCell::new(None) };
}

pub(crate) struct HostIoGuard {
    size: u64,
}

impl HostIoGuard {
    pub(crate) fn register(source: &JsValue, write: &JsValue) -> Result<Self, JsValue> {
        if ACTIVE.with(Cell::get) {
            return Err(js_error(
                ErrorCode::InvalidArgument,
                "convert_stream cannot be called recursively on one WASM instance",
            ));
        }
        ACTIVE.with(|active| active.set(true));
        let result = Self::register_active(source, write);
        if result.is_err() {
            ACTIVE.with(|active| active.set(false));
        }
        result
    }

    fn register_active(source: &JsValue, write_value: &JsValue) -> Result<Self, JsValue> {
        let write_callback = write_value
            .clone()
            .dyn_into::<js_sys::Function>()
            .map_err(|_value| js_error(ErrorCode::InvalidArgument, "write must be a function"))?;
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
        CALLBACKS.with(|slot| {
            *slot.borrow_mut() = Some(Callbacks {
                source: source.clone(),
                read_at,
                write: write_callback,
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
        ACTIVE.with(|active| active.set(false));
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
        if buf.is_empty() || self.position == self.size {
            return Ok(0);
        }
        let remaining = self.size.saturating_sub(self.position);
        let requested = buf
            .len()
            .min(MAX_TRANSFER_BYTES)
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let callbacks = callbacks()?;
        let value = callbacks
            .read_at
            .call2(
                &callbacks.source,
                &JsValue::from_f64(self.position as f64),
                &JsValue::from_f64(requested as f64),
            )
            .map_err(|value| host_failure(ErrorCode::Io, exception_message(&value)))?;
        if is_promise(&value) {
            return Err(host_failure(
                ErrorCode::InvalidArgument,
                String::from("source.readAt must return synchronously"),
            ));
        }
        let bytes = value.dyn_into::<js_sys::Uint8Array>().map_err(|_value| {
            host_failure(
                ErrorCode::InvalidArgument,
                String::from("source.readAt must return a Uint8Array"),
            )
        })?;
        let actual = usize::try_from(bytes.length())
            .map_err(|error| host_failure(ErrorCode::InvalidArgument, error.to_string()))?;
        if actual > requested {
            return Err(host_failure(
                ErrorCode::InvalidArgument,
                String::from("source.readAt returned more bytes than requested"),
            ));
        }
        if actual == 0 {
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
        bytes.copy_to(destination);
        self.position = self
            .position
            .checked_add(
                u64::try_from(actual)
                    .map_err(|error| host_failure(ErrorCode::InvalidArgument, error.to_string()))?,
            )
            .ok_or_else(|| {
                host_failure(
                    ErrorCode::InvalidArgument,
                    String::from("source position overflowed"),
                )
            })?;
        Ok(actual)
    }
}

impl Seek for HostReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let next = match pos {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.size)
                .checked_add(i128::from(offset))
                .ok_or_else(|| {
                    host_failure(
                        ErrorCode::InvalidArgument,
                        String::from("seek offset overflowed"),
                    )
                })?,
            SeekFrom::Current(offset) => i128::from(self.position)
                .checked_add(i128::from(offset))
                .ok_or_else(|| {
                    host_failure(
                        ErrorCode::InvalidArgument,
                        String::from("seek offset overflowed"),
                    )
                })?,
        };
        if !(0..=i128::from(MAX_SAFE_INTEGER)).contains(&next) {
            return Err(host_failure(
                ErrorCode::InvalidArgument,
                String::from("seek offset must be a nonnegative safe integer"),
            ));
        }
        self.position = u64::try_from(next)
            .map_err(|error| host_failure(ErrorCode::InvalidArgument, error.to_string()))?;
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
        let result = callbacks
            .write
            .call1(&JsValue::UNDEFINED, chunk.as_ref())
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
        if self.buffer.borrow().len() == MAX_TRANSFER_BYTES {
            self.flush()?;
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

fn host_failure(code: ErrorCode, message: String) -> io::Error {
    FAILURE.with(|failure| {
        let mut recorded = failure.borrow_mut();
        if recorded.is_none() {
            *recorded = Some((code, message.clone()));
        }
    });
    io::Error::other(message)
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
