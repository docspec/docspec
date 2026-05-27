//! Callback-invocation-count tests for BUG-6.
//!
//! Run via `wasm-pack test --node` (real JS engine required for callbacks).
//! Native `cargo test` compiles these but does not run them.

#![cfg(target_arch = "wasm32")]

use core::cell::Cell;
use core::cell::RefCell;
use core::fmt::Write as _;
use std::rc::Rc;

use wasm_bindgen::prelude::{Closure, JsCast as _, JsValue};
use wasm_bindgen_test::wasm_bindgen_test;
use wasm_bindgen_test::wasm_bindgen_test_configure;

use docspec_wasm::{convert_markdown_to_blocknote, convert_markdown_to_blocknote_streaming};

wasm_bindgen_test_configure!(run_in_node_experimental);

fn large_markdown() -> String {
    let mut md = String::with_capacity(200_000);
    for i in 0..5_000u32 {
        _ = write!(md, "# Heading {i}\n\nParagraph text for section {i}.\n\n");
    }
    md
}

#[wasm_bindgen_test]
fn streaming_invokes_callback_multiple_times() {
    let count = Rc::new(Cell::new(0u32));
    let count_clone = Rc::clone(&count);

    let closure_fn: Box<dyn FnMut(JsValue)> = Box::new(move |_chunk: JsValue| {
        count_clone.set(count_clone.get().saturating_add(1u32));
    });
    let callback = Closure::wrap(closure_fn);

    let markdown = large_markdown();
    let result =
        convert_markdown_to_blocknote_streaming(&markdown, callback.as_ref().unchecked_ref());
    drop(callback);

    assert!(
        result.is_ok(),
        "streaming conversion must succeed: {result:?}"
    );
    let chunk_count = count.get();
    assert!(
        chunk_count >= 2,
        "streaming variant must invoke callback at least 2 times; got {chunk_count}"
    );
}

#[wasm_bindgen_test]
fn buffered_and_streaming_produce_equivalent_output() {
    let markdown = "# Hello\n\nWorld\n";

    let buffered_result = convert_markdown_to_blocknote(markdown);
    assert!(
        buffered_result.is_ok(),
        "buffered conversion must succeed: {buffered_result:?}"
    );
    let buffered = buffered_result.unwrap_or_default();

    let chunks: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let chunks_clone = Rc::clone(&chunks);

    let closure_fn: Box<dyn FnMut(JsValue)> = Box::new(move |chunk: JsValue| {
        let arr: js_sys::Uint8Array = chunk.unchecked_into();
        chunks_clone.borrow_mut().extend_from_slice(&arr.to_vec());
    });
    let callback = Closure::wrap(closure_fn);

    let result =
        convert_markdown_to_blocknote_streaming(markdown, callback.as_ref().unchecked_ref());
    drop(callback);

    assert!(
        result.is_ok(),
        "streaming conversion must succeed: {result:?}"
    );

    let streaming_bytes = chunks.borrow().clone();
    let streaming_result = String::from_utf8(streaming_bytes);
    assert!(
        streaming_result.is_ok(),
        "streaming output must be valid UTF-8"
    );
    let streaming = streaming_result.unwrap_or_default();

    assert_eq!(
        buffered, streaming,
        "buffered and streaming variants must produce identical JSON"
    );
}

#[wasm_bindgen_test]
fn streaming_callback_exception_propagates_as_err() {
    let throwing_fn =
        js_sys::Function::new_no_args("throw new Error('test exception from BUG-6 test')");
    let markdown = "# Hello\n\nWorld\n";
    let result = convert_markdown_to_blocknote_streaming(markdown, &throwing_fn);
    assert!(
        result.is_err(),
        "a throwing callback must cause streaming conversion to return Err"
    );
}
