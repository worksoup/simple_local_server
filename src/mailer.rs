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

use getset2::Getset2;
use lettre::{Message, SmtpTransport, Transport, transport::smtp::authentication::Credentials};

use crate::config::EMailAccountConfig;

#[derive(Debug, Clone, Getset2)]
#[getset2(get_ref(pub))]
pub struct EMailer {
    sender_addr: lettre::Address,
    mailer: SmtpTransport,
}

impl EMailer {
    #[inline]
    pub fn new(
        EMailAccountConfig {
            server,
            uname,
            password,
            port,
        }: EMailAccountConfig,
    ) -> Result<Self, lettre::transport::smtp::Error> {
        let mut builder = SmtpTransport::from_url(server.as_str())?;
        if let Some(port) = port {
            builder = builder.port(port);
        }
        let sender_addr = uname
            .0
            .parse()
            .inspect_err(|e| {
                tracing::warn!(
                    "用户名（长度为 {}）无法解析为发送方信息：`{e}`, 发送可能失败。",
                    uname.0.len()
                )
            })
            .unwrap_or("no-replay@sl.server".parse().expect("字面值构建失败。"));
        let cred = Credentials::new(uname.0.clone(), password.0);
        Ok(Self {
            sender_addr,
            mailer: builder.credentials(cred).build(),
        })
    }

    #[inline]
    pub fn send(
        &self,
        email: &Message,
    ) -> Result<lettre::transport::smtp::response::Response, lettre::transport::smtp::Error> {
        self.mailer.send(email)
    }
}
