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

//! 跟踪器列表(Tracker List)服务模块
//! 从多个公开的Tracker列表源获取并合并Tracker地址，提供缓存和HTTP接口
//! 主要功能包括：
//! 1. 从配置的URL源获取Tracker列表
//! 2. 对获取的数据进行缓存以减少外部请求
//! 3. 提供HTTP接口返回合并后的Tracker列表

use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use actix_web::web;
use moka::future::Cache;
use tracing::{debug, error, info, instrument, warn};

use crate::{config::TrackerMergerConfig, error::ResultUtils};

/// 应用程序状态结构体
/// 包含所有Tracker列表源的缓存实例
/// 每个URL源对应一个独立的缓存
#[derive(Clone)]
pub struct State {
    /// URL源到其缓存的映射
    /// Key: Tracker列表源的URL
    /// Value: 缓存实例，缓存键为()，值为从该源获取的Tracker URL集合
    caches: HashMap<url::Url, Cache<(), HashSet<url::Url>>>,
}

impl State {
    /// 根据配置创建新的状态实例
    /// 为每个配置的URL源初始化缓存
    #[instrument(name = "初始化Tracker列表状态", skip(config))]
    pub fn new(config: &TrackerMergerConfig) -> Self {
        info!("正在初始化Tracker列表状态");
        let mut caches = HashMap::new();
        let mut cache_count = 0;

        // 为每个URL源创建缓存实例
        for (url, ttl) in config.urls() {
            // 使用URL特定的TTL或全局默认TTL
            let ttl = ttl.unwrap_or(config.ttl().clone());
            // 构建缓存：设置TTL和最大容量
            let cache = Cache::builder()
                .time_to_live(ttl) // 缓存生存时间
                .max_capacity(25) // 最大缓存容量
                .build();

            debug!(
                "为Tracker源 {} 创建缓存，TTL={}秒，最大容量=25",
                url,
                ttl.as_secs()
            );

            caches.insert(url.clone(), cache);
            cache_count += 1;
        }

        info!(
            "Tracker列表状态初始化完成，共创建 {} 个缓存实例",
            cache_count
        );
        Self { caches }
    }

    /// 获取指定URL源的Tracker列表
    /// 优先从缓存读取，缓存未命中则从网络获取并缓存结果
    ///
    /// # 参数
    /// - `client`: HTTP客户端实例
    /// - `url`: Tracker列表源的URL
    ///
    /// # 返回值
    /// - `Ok(HashSet<Url>)`: 从该源解析出的有效Tracker URL集合
    /// - `Err(reqwest::Error)`: HTTP请求或解析失败
    #[instrument(
        name = "获取Tracker列表",
        skip(self, client),
        fields(url = %url)
    )]
    pub async fn get_tracker_list(
        &self,
        client: &reqwest::Client,
        url: &url::Url,
    ) -> Result<HashSet<url::Url>, reqwest::Error> {
        // 检查是否有该URL源的缓存配置
        if let Some(cache) = self.caches.get(url) {
            // 尝试从缓存获取数据
            if let Some(cached) = cache.get(&()).await {
                debug!("缓存命中，从缓存获取到 {} 个Tracker地址", cached.len());
                // 缓存命中，直接返回缓存数据
                return Ok(cached);
            }

            debug!("缓存未命中，将从网络获取数据");
        } else {
            warn!("请求的Tracker源 {} 没有缓存配置", url);
        }

        info!("开始从 {} 获取Tracker列表", url);

        // 缓存未命中，从网络获取
        let mut result = HashSet::new();
        let mut valid_count = 0;
        let mut invalid_count = 0;

        // 发送HTTP GET请求获取Tracker列表文本
        let res = client.get(url.clone()).send().await?;

        // 读取响应文本，使用log_ok记录可能的错误但继续处理
        if let Some(text) = res.text().await.log_ok() {
            debug!("从 {} 获取数据", url);

            // 逐行处理文本
            for (line_num, line) in text.lines().enumerate() {
                let trimmed = line.trim();

                // 跳过空行，尝试将每行解析为URL
                if !trimmed.is_empty() {
                    match url::Url::from_str(trimmed) {
                        Ok(url) => {
                            debug!("第 {} 行解析成功: {}", line_num + 1, url);
                            result.insert(url);
                            valid_count += 1;
                        }
                        Err(e) => {
                            debug!("第 {} 行解析失败 '{}': {}", line_num + 1, trimmed, e);
                            invalid_count += 1;
                        }
                    }
                }
            }
        }

        info!(
            "从 {} 获取完成，有效地址: {}，无效地址: {}",
            url, valid_count, invalid_count
        );

        // 如果该URL源有缓存配置，将结果缓存起来
        if let Some(cache) = self.caches.get(url) {
            debug!("将结果缓存，有效期根据配置");
            cache.insert((), result.clone()).await;
        }

        Ok(result)
    }
}

#[instrument(
    name = "处理Tracker列表请求",
    skip(config, state),
    fields(config_sources = config.urls().len())
)]
#[actix_web::get("/tracker_list")]
async fn tracker_list(
    config: web::Data<TrackerMergerConfig>,
    state: web::Data<State>,
) -> impl actix_web::Responder {
    info!("收到Tracker列表请求");

    // 创建HTTP客户端
    let client = reqwest::Client::new();
    let mut requests = Vec::new();

    // 为每个配置的URL源创建异步获取任务
    for url in config.get_ref().urls().keys() {
        debug!("准备从 {} 获取Tracker列表", url);
        let job = state.get_tracker_list(&client, url);
        requests.push(job);
    }

    info!("开始并发获取 {} 个Tracker源的数据", requests.len());

    // 并发执行所有获取任务
    let results = futures::future::join_all(requests).await;

    let mut total_trackers = HashSet::new();
    let mut successful_sources = 0;
    let mut failed_sources = 0;

    for (i, res) in results.into_iter().enumerate() {
        match res {
            Ok(trackers) => {
                let count = trackers.len();
                debug!("第 {} 个源获取成功，获取到 {} 个Tracker地址", i + 1, count);
                for url in trackers {
                    total_trackers.insert(url.to_string());
                }
                successful_sources += 1;
            }
            Err(e) => {
                error!("第 {} 个源获取失败: {}", i + 1, e);
                failed_sources += 1;
            }
        }
    }

    info!(
        "所有Tracker源获取完成，成功: {}，失败: {}，去重后总数: {}",
        successful_sources,
        failed_sources,
        total_trackers.len()
    );

    if total_trackers.is_empty() && failed_sources > 0 {
        warn!("所有Tracker源获取都失败了，返回空列表");
    }

    // 将结果集合转换为纯文本格式，每行一个URL
    let result_text = total_trackers
        .into_iter()
        .fold(String::new(), |r, l| r + &l + "\n");

    info!("返回 {} 字节的Tracker列表数据", result_text.len());

    // 返回HTTP响应
    actix_web::HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(result_text)
}
