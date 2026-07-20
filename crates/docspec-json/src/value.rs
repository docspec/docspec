//! Trait and impls for values writable through a [`JsonBackend`].

use crate::JsonBackend;
use docspec_core::Result;

/// Marker type representing a JSON `null` value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Null;

/// A value that can be written through a [`JsonBackend`].
pub trait WriteVal {
    /// Write `self` to `backend`.
    ///
    /// # Errors
    ///
    /// Returns any error produced while writing the value.
    fn write_to<B>(self, backend: &mut B) -> Result<()>
    where
        B: JsonBackend;
}

impl WriteVal for &str {
    #[inline]
    fn write_to<B>(self, backend: &mut B) -> Result<()>
    where
        B: JsonBackend,
    {
        backend.write_string(self)
    }
}

impl WriteVal for bool {
    #[inline]
    fn write_to<B>(self, backend: &mut B) -> Result<()>
    where
        B: JsonBackend,
    {
        backend.write_bool(self)
    }
}

impl WriteVal for u8 {
    #[inline]
    fn write_to<B>(self, backend: &mut B) -> Result<()>
    where
        B: JsonBackend,
    {
        backend.write_number(u32::from(self))
    }
}

impl WriteVal for u32 {
    #[inline]
    fn write_to<B>(self, backend: &mut B) -> Result<()>
    where
        B: JsonBackend,
    {
        backend.write_number(self)
    }
}

impl WriteVal for Null {
    #[inline]
    fn write_to<B>(self, backend: &mut B) -> Result<()>
    where
        B: JsonBackend,
    {
        backend.write_null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docspec_core::Error;

    #[derive(Default)]
    struct MockBackend {
        fail: bool,
        last_bool: Option<bool>,
        last_call: &'static str,
        last_number: Option<u32>,
        last_string: Option<String>,
    }

    impl crate::JsonBackend for MockBackend {
        type Output = ();

        fn begin_array(&mut self) -> docspec_core::Result<()> {
            Ok(())
        }

        fn begin_object(&mut self) -> docspec_core::Result<()> {
            Ok(())
        }

        fn end_array(&mut self) -> docspec_core::Result<()> {
            Ok(())
        }

        fn end_object(&mut self) -> docspec_core::Result<()> {
            Ok(())
        }

        fn finish(self) -> docspec_core::Result<()> {
            Ok(())
        }

        fn write_bool(&mut self, b: bool) -> docspec_core::Result<()> {
            self.last_call = "write_bool";
            self.last_bool = Some(b);
            Ok(())
        }

        fn write_name(&mut self, _: &str) -> docspec_core::Result<()> {
            Ok(())
        }

        fn write_null(&mut self) -> docspec_core::Result<()> {
            self.last_call = "write_null";
            Ok(())
        }

        fn write_number(&mut self, n: u32) -> docspec_core::Result<()> {
            self.last_call = "write_number";
            self.last_number = Some(n);
            Ok(())
        }

        fn write_string(&mut self, s: &str) -> docspec_core::Result<()> {
            self.last_call = "write_string";
            self.last_string = Some(s.to_string());
            if self.fail {
                Err(Error::Other {
                    message: "mock error".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn mock_backend_container_methods_callable() {
        let mut b = MockBackend::default();
        assert!(b.begin_array().is_ok());
        assert!(b.begin_object().is_ok());
        assert!(b.end_array().is_ok());
        assert!(b.end_object().is_ok());
        assert!(b.write_name("k").is_ok());
    }

    #[test]
    fn mock_backend_finish_callable() {
        let b = MockBackend::default();
        assert!(b.finish().is_ok());
    }

    #[test]
    fn write_val_bool_false_calls_write_bool_with_false() {
        let mut b = MockBackend::default();
        assert!(false.write_to(&mut b).is_ok());
        assert_eq!(b.last_call, "write_bool");
        assert_eq!(b.last_bool, Some(false));
    }

    #[test]
    fn write_val_bool_true_calls_write_bool_with_true() {
        let mut b = MockBackend::default();
        assert!(true.write_to(&mut b).is_ok());
        assert_eq!(b.last_call, "write_bool");
        assert_eq!(b.last_bool, Some(true));
    }

    #[test]
    fn write_val_null_calls_write_null() {
        let mut b = MockBackend::default();
        assert!(Null.write_to(&mut b).is_ok());
        assert_eq!(b.last_call, "write_null");
    }

    #[test]
    fn write_val_propagates_backend_error() {
        let mut b = MockBackend {
            fail: true,
            ..MockBackend::default()
        };
        assert!("hello".write_to(&mut b).is_err());
    }

    #[test]
    fn write_val_str_calls_write_string() {
        let mut b = MockBackend::default();
        assert!("hello".write_to(&mut b).is_ok());
        assert_eq!(b.last_call, "write_string");
        assert_eq!(b.last_string.as_deref(), Some("hello"));
    }

    #[test]
    fn write_val_u8_calls_write_number() {
        let mut b = MockBackend::default();
        let n: u8 = 42;
        assert!(n.write_to(&mut b).is_ok());
        assert_eq!(b.last_call, "write_number");
        assert_eq!(b.last_number, Some(42));
    }

    #[test]
    fn write_val_u32_calls_write_number() {
        let mut b = MockBackend::default();
        let n: u32 = 42;
        assert!(n.write_to(&mut b).is_ok());
        assert_eq!(b.last_call, "write_number");
        assert_eq!(b.last_number, Some(42));
    }
}
