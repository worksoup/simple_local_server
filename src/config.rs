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

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tracker_merger;
use crate::utils::SensitiveString;

#[derive(Debug, Clone, Serialize, Deserialize, getset2::Getset2, derive_builder::Builder)]
#[getset2(get_ref(pub))]
#[builder(
    private,
    name = DeSummaryConfig,
    derive(Debug, serde::Deserialize),
    build_fn(private, error = std::convert::Infallible),
)]
#[serde(from = "DeSummaryConfig")]
pub struct SummaryConfig {
    #[builder(default = std::net::SocketAddr::V4(std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(127, 0, 0, 1), 5139))
    )]
    pub(crate) addr: std::net::SocketAddr,
    #[builder(
        default = Some(PathBuf::from("logs")),
        setter(strip_option))]
    pub(crate) log_dir: Option<PathBuf>,
    #[builder(default)]
    pub(crate) tracker_list_config: tracker_merger::Config,
    #[builder(
        setter(strip_option),
        field(ty = "Option<crate::mailer::DeEMailAccountConfig>",
        build = build_summary_config_email_account(self)
    ))]
    pub(crate) email_account: Option<crate::mailer::EMailAccountConfig>,
    #[builder(
        setter(strip_option),
        field(ty = "Option<DeTiebaSignConfig>",
        build = build_summary_config_tieba_sign_config(self)
    ))]
    pub(crate) tieba_sign_config: Option<TiebaSignConfig>,
}
fn build_summary_config_email_account(
    sc: &DeSummaryConfig,
) -> Option<crate::mailer::EMailAccountConfig> {
    sc.email_account
        .as_ref()
        .and_then(|builder| builder.build().inspect_err(|e| tracing::error!("{e}")).ok())
}
fn build_summary_config_tieba_sign_config(sc: &DeSummaryConfig) -> Option<TiebaSignConfig> {
    sc.tieba_sign_config
        .clone()
        .and_then(|builder| builder.build().inspect_err(|e| tracing::error!("{e}")).ok())
}
impl core::default::Default for SummaryConfig {
    #[inline]
    fn default() -> Self {
        DeSummaryConfig::default().build().unwrap()
    }
}

impl From<DeSummaryConfig> for SummaryConfig {
    #[inline]
    fn from(de: DeSummaryConfig) -> Self {
        de.build().unwrap()
    }
}

#[derive(Debug, Clone, Serialize, getset2::Getset2, derive_builder::Builder)]
#[getset2(get_ref(pub))]
#[builder(
    private,
    name = DeTiebaSignConfig,
    derive(Debug, serde::Deserialize),
    build_fn(private),
)]
pub struct TiebaSignConfig {
    bduss: SensitiveString,
    #[builder(default)]
    dont_ntfy: Vec<String>,
    #[builder(field(ty = "Option<String>", build = self.sign_result_send_to.as_ref().and_then(|addr| addr.parse().inspect_err(|e| {tracing::error!("配置的消息接收电邮地址不合法：`{e}`.")}).ok())))]
    #[serde(serialize_with = "sign_result_send_to_serialize")]
    sign_result_send_to: Option<lettre::Address>,
}
fn sign_result_send_to_serialize<S>(addr: &Option<lettre::Address>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(addr) = addr {
        s.serialize_str(&addr.to_string())
    } else {
        s.serialize_none()
    }
}

impl TiebaSignConfig {
    #[inline]
    pub const fn new(
        bduss: SensitiveString,
        dont_ntfy: Vec<String>,
        sign_result_send_to: Option<lettre::Address>,
    ) -> Self {
        Self {
            bduss,
            dont_ntfy,
            sign_result_send_to,
        }
    }
}
