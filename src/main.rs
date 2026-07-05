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

use std::{fs, path::PathBuf};

use actix_web::{App, HttpServer, web};
use clap::Parser;
use tracing_actix_web::TracingLogger;

use crate::mailer::EMailer;
use crate::tieba_sign::TiebaSignDaemon;
use crate::{config::SummaryConfig, tracker_merger::tracker_list};

mod alacritty_theme_updater;
mod tieba_sign;
mod tracker_merger;

mod config;
pub mod error;
mod logger;
mod mailer;
mod test_utils;
mod utils;

/// 命令行参数
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 配置文件路径。
    #[arg(short, default_value = "sl-server.toml")]
    config: PathBuf,
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config_path = args.config;

    let logger = logger::TerminalOnlyLogger::new();
    let config = fs::read_to_string(&config_path).unwrap_or_else(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            tracing::warn!(
                "无法读取配置文件 `{}`：`{e}`，将生成默认配置。",
                config_path.display()
            );
            let config =
                toml::to_string_pretty(&SummaryConfig::default()).expect("默认配置无法被序列化。");
            fs::write(&config_path, &config).unwrap_or_else(|e| {
                tracing::warn!(
                    "无法写入文件 `{}`：`{e}`，将继续以默认配置运行。",
                    config_path.display()
                )
            });
            config
        }
        _ => {
            tracing::error!("无法读取配置文件 `{}`：`{e}`。", config_path.display());
            panic!()
        }
    });
    let config = toml::from_str(&config).expect("无法解析配置文件");
    tracing::info!("以`{config:?}`为配置启动。");
    let SummaryConfig {
        addr,
        log_dir,
        tracker_list_config,
        email_account,
        tieba_sign_config,
    } = config;
    let mailer = web::Data::new(email_account.map(|ea| EMailer::new(ea)));
    if mailer.is_none() {
        tracing::warn!("未配置电邮发送服务。");
    }
    let tieba_sign_daemon = if let Some(tieba_sign_config) = tieba_sign_config {
        Some(TiebaSignDaemon::run(tieba_sign_config, mailer.clone()).await)
    } else {
        tracing::warn!("未配置贴吧签到服务。");
        None
    };
    let logger = if let Some(log_dir) = &log_dir {
        logger.set_log_dir(log_dir)
    } else {
        Err(logger)
    };
    let tracker_list_config = web::Data::new(tracker_list_config);
    let tracker_list_state = web::Data::new(tracker_merger::State::new(&tracker_list_config));
    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(tracker_list_config.clone())
            .app_data(tracker_list_state.clone())
            .service(tracker_list)
    })
    .bind(addr)?
    .run()
    .await?;
    drop(logger);
    drop(tieba_sign_daemon);
    Ok(())
}
