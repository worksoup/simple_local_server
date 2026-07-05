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

use std::{ffi::OsStr, fs, path::Path};

use tracing::dispatcher::DefaultGuard;
use tracing_appender::{
    non_blocking::{NonBlocking, WorkerGuard},
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{
    Layer, filter::FilterFn, fmt::writer::MakeWriterExt, layer::SubscriberExt,
};

pub struct TerminalOnlyLogger {
    guard: DefaultGuard,
    log_level: tracing::Level,
}

pub struct Logger {
    _common_log_guard: WorkerGuard,
    _warn_and_error_log_guard: WorkerGuard,
    _debug_log_guard: WorkerGuard,
    _trace_log_guard: WorkerGuard,
}

impl TerminalOnlyLogger {
    pub fn new() -> Self {
        tracing_log::LogTracer::init().expect("初始化 tracing-log 日志追踪器失败。");
        // 终端中输出的日志等级。
        let terminal_log_level = std::env::var("SL_TERMINAL_LOG")
            .map(|s| match s.as_str() {
                "trace" => tracing::Level::TRACE,
                "debug" => tracing::Level::DEBUG,
                "info" => tracing::Level::INFO,
                "warn" => tracing::Level::WARN,
                "error" => tracing::Level::ERROR,
                _ => tracing::Level::INFO,
            })
            .unwrap_or(tracing::Level::INFO);
        let filter = Self::get_filter(terminal_log_level);
        // 先在终端中输出日志。
        let terminal_only_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr.with_max_level(terminal_log_level))
            .with_file(false)
            .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
            .with_filter(filter.clone());
        let terminal_only_subscriber =
            tracing_subscriber::Registry::default().with(terminal_only_layer);
        let terminal_only_subscriber_guard =
            tracing::subscriber::set_default(terminal_only_subscriber);
        Self {
            guard: terminal_only_subscriber_guard,
            log_level: terminal_log_level,
        }
    }
    pub fn set_log_dir(self, log_dir: impl AsRef<Path>) -> Result<Logger, TerminalOnlyLogger> {
        let log_dir = log_dir.as_ref();
        if !log_dir.exists() {
            if let Err(e) = fs::create_dir_all(log_dir) {
                tracing::warn!("无法创建日志目录 `{}`: `{}`.", log_dir.display(), e);
                return Err(self);
            }
        } else if !log_dir.is_dir() {
            tracing::warn!("日志路径已存在但不是目录: `{}`.", log_dir.display());
            return Err(self);
        }
        let Self {
            guard: guards,
            log_level: terminal_log_level,
        } = self;
        // 此时初始化文件日志。
        // 分为 log (info, warn, error), warn_and_error (literally), debug, trace.
        let (common_log, common_log_guard) = Self::log_file_appender(log_dir, "common.log");
        let (warn_and_error_log, warn_and_error_log_guard) =
            Self::log_file_appender(log_dir, "warn_and_error.log");
        let (debug_log, debug_log_guard) = Self::log_file_appender(log_dir, "debug.log");
        let (trace_log, trace_log_guard) = Self::log_file_appender(log_dir, "trace.log");
        let mk_writer = common_log
            .with_max_level(tracing::Level::INFO)
            .and(
                warn_and_error_log
                    .with_min_level(tracing::Level::WARN)
                    .with_max_level(tracing::Level::WARN),
            )
            .and(
                debug_log
                    .with_min_level(tracing::Level::DEBUG)
                    .with_max_level(tracing::Level::DEBUG),
            )
            .and(
                trace_log
                    .with_min_level(tracing::Level::TRACE)
                    .with_max_level(tracing::Level::TRACE),
            );
        let filter = Self::get_filter(terminal_log_level);
        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(mk_writer)
            .with_file(false)
            .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
            .with_filter(filter.clone());
        let terminal_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr.with_max_level(terminal_log_level))
            .with_file(false)
            .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
            .with_filter(filter);
        let subscriber = tracing_subscriber::Registry::default()
            .with(file_layer)
            .with(terminal_layer);
        // 初始化完成，使用新的日志输出。
        drop(guards);
        tracing::subscriber::set_global_default(subscriber)
            .expect("初始化 tracing 全局订阅器失败。");
        Ok(Logger {
            _common_log_guard: common_log_guard,
            _warn_and_error_log_guard: warn_and_error_log_guard,
            _debug_log_guard: debug_log_guard,
            _trace_log_guard: trace_log_guard,
        })
    }

    /// 日志文件，每小时更换新的写入器。
    ///
    /// 返回 [`NonBlocking`] 与对应的 [`WorkerGuard`].
    ///
    /// NonBlocking 不会立即写入文件，故需要 WorkerGuard.
    /// NonBlocking 最后会被置入一个全局静态变量中，导致其 drop 不会运行（应该是这样）。所以不能期望让它本身去完成最后的 flush.
    /// 而返回一个 guard, 放在栈上，程序无论何时退出，它总会被清理，清理时完成 flush 即可。
    ///
    /// 故，此 guard 须保留至 main 末尾（对于多线程，我们假设 main 结束后不会产生任何新的日志。实际上，main 如果不等待其它线程结束，各类资源都不太好清理，而本程序也不应容许此类情况）。
    fn log_file_appender(
        log_dir: impl AsRef<Path>,
        p: impl AsRef<Path>,
    ) -> (NonBlocking, WorkerGuard) {
        let log_dir = log_dir.as_ref();
        let p = p.as_ref();
        let ext = p.extension();
        let mut filename = p.as_os_str();
        if let Some(ext) = ext {
            let dot: &OsStr = ".".as_ref();
            // SAFETY: encoded_bytes 移除多个 ext 的 encoded_bytes 后，仍满足 from_encoded_bytes_unchecked 要求。
            filename = unsafe {
                OsStr::from_encoded_bytes_unchecked(
                    filename
                        .as_encoded_bytes()
                        .strip_suffix(ext.as_encoded_bytes())
                        // NOTE: panic: unwrap: 扩展名存在，故不会失败。
                        .unwrap()
                        .strip_suffix(dot.as_encoded_bytes())
                        // NOTE: panic: unwrap: 扩展名存在，故不会失败。
                        .unwrap(),
                )
            };
        }
        let hourly_appender = RollingFileAppender::builder()
            .rotation(Rotation::HOURLY)
            .filename_prefix(filename.to_str().unwrap_or("default_log"))
            .filename_suffix(ext.and_then(|ext| ext.to_str()).unwrap_or("log"))
            .build(log_dir)
            .expect("initializing rolling file appender failed");
        tracing_appender::non_blocking(hourly_appender)
    }

    #[inline]
    fn get_filter(
        terminal_log_level: tracing::Level,
    ) -> FilterFn<impl Fn(&tracing::Metadata<'_>) -> bool + Clone> {
        FilterFn::new(move |metadata| {
            metadata.level() != &tracing::Level::INFO
                || terminal_log_level != tracing::Level::INFO
                || !metadata.target().starts_with("i18n_embed")
        })
    }
}
