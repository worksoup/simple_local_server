// MIT License
//
// Copyright (c) 2026 worksoup <https://github.com/worksoup/>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#[inline]
#[track_caller]
pub fn log_none<T>(e: impl std::fmt::Debug) -> Option<T> {
    let caller = core::panic::Location::caller();
    tracing::warn!("`{caller}`: {e:?}, 将使用空值。",);
    None
}

#[inline]
#[track_caller]
pub fn log_panic<T>(e: impl std::fmt::Debug) -> T {
    let caller = core::panic::Location::caller();
    tracing::error!("`{caller}`: {e:?}.");
    panic!();
}

#[inline]
#[track_caller]
pub fn log_expect<T>(e: impl std::fmt::Debug, msg: impl std::fmt::Display) -> T {
    let caller = core::panic::Location::caller();
    tracing::error!("`{caller}`: {msg}: {e:?}.");
    panic!();
}

#[inline]
#[track_caller]
pub fn log_default<T: Default>(e: impl std::fmt::Debug) -> T {
    let caller = core::panic::Location::caller();
    tracing::warn!("`{caller}`: {e:?}, 将使用默认值。",);
    T::default()
}
#[inline]
#[track_caller]
pub fn log_or<T>(e: impl std::fmt::Debug, val: T) -> T {
    let caller = core::panic::Location::caller();
    tracing::warn!("`{caller}`: {e:?}, 将使用回退值。",);
    val
}
#[inline]
#[track_caller]
pub fn log_or_else<T, E: std::fmt::Debug, F: FnOnce(E) -> T>(e: E, f: F) -> T {
    let caller = core::panic::Location::caller();
    tracing::warn!("`{caller}`: {e:?}, 将使用回退值。",);
    f(e)
}

pub trait ResultUtils<T, E> {
    fn log_ignore(self);
    fn log_panic(self) -> T;
    fn log_expect(self, msg: &str) -> T;
    fn log_default(self) -> T
    where
        T: Default;
    fn log_ok(self) -> Option<T>;
    fn ok_or_else<F: FnOnce(E) -> Option<T>>(self, f: F) -> Option<T>;
    fn log_or(self, val: T) -> T;
    fn log_or_else<F: FnOnce(E) -> T>(self, f: F) -> T;
}
impl<T, E: std::fmt::Debug> ResultUtils<T, E> for Result<T, E> {
    #[inline]
    #[track_caller]
    fn log_panic(self) -> T {
        self.unwrap_or_else(log_panic)
    }
    #[inline]
    #[track_caller]
    fn log_expect(self, msg: &str) -> T {
        self.unwrap_or_else(|e| log_expect(e, msg))
    }

    #[inline]
    #[track_caller]
    fn log_default(self) -> T
    where
        T: Default,
    {
        self.unwrap_or_else(log_default)
    }

    #[inline]
    #[track_caller]
    fn log_ok(self) -> Option<T> {
        self.ok_or_else(log_none)
    }

    #[inline]
    #[track_caller]
    fn log_ignore(self) {
        let caller = core::panic::Location::caller();
        match self {
            Ok(_) => tracing::debug!("`{caller}`: 值已被忽略。",),
            Err(e) => tracing::warn!("`{caller}`: 忽略错误：{e:?}。",),
        }
    }

    #[inline]
    fn ok_or_else<F: FnOnce(E) -> Option<T>>(self, f: F) -> Option<T> {
        match self {
            Ok(x) => Some(x),
            Err(e) => f(e),
        }
    }

    #[inline]
    #[track_caller]
    fn log_or(self, val: T) -> T {
        self.unwrap_or_else(|e| log_or(e, val))
    }

    #[inline]
    #[track_caller]
    fn log_or_else<F: FnOnce(E) -> T>(self, f: F) -> T {
        self.unwrap_or_else(|e| log_or_else(e, f))
    }
}
