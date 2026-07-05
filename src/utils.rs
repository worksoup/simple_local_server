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

use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use tracing::field::{Field, Visit};

pub mod url_serde {
    use url::Url;

    pub(crate) fn serialize<S>(url: &Url, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        s.serialize_str(url.as_str())
    }
    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Url, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(try_from = "String")]
        pub struct DeUrl(Url);
        impl TryFrom<String> for DeUrl {
            type Error = url::ParseError;

            #[inline]
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Ok(Self(Url::parse(&value)?))
            }
        }
        <DeUrl as serde::Deserialize>::deserialize(deserializer).map(|de_url| de_url.0)
    }
}

#[derive(Clone, Copy, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct SensitiveStr<'a>(pub(crate) &'a str);
impl<'a> Display for SensitiveStr<'a> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt("<敏感信息>", f)
    }
}
impl<'a> Debug for SensitiveStr<'a> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt("<敏感信息>", f)
    }
}
impl<'a> From<&'a str> for SensitiveStr<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self(value)
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct SensitiveString(pub(crate) String);

impl SensitiveString {
    #[inline]
    pub const fn as_ref(&self) -> SensitiveStr<'_> {
        SensitiveStr(self.0.as_str())
    }
}

impl Display for SensitiveString {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt("<敏感信息>", f)
    }
}
impl Debug for SensitiveString {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt("<敏感信息>", f)
    }
}
impl From<String> for SensitiveString {
    #[inline]
    fn from(value: String) -> Self {
        Self(value)
    }
}
