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

use std::{collections::HashMap, path::PathBuf, time::Duration};

use getset2::Getset2;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

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
    pub(crate) tracker_list_config: TrackerMergerConfig,
    #[builder(
        setter(strip_option),
        field(ty = "Option<DeEMailAccountConfig>",
        build = build_summary_config_email_account(self)
    ))]
    pub(crate) email_account: Option<EMailAccountConfig>,
    #[builder(
        setter(strip_option),
        field(ty = "Option<DeTiebaSignConfig>",
        build = build_summary_config_tieba_sign_config(self)
    ))]
    pub(crate) tieba_sign_config: Option<TiebaSignConfig>,
}

/// 带有TTL配置的URL结构体
/// 用于序列化/反序列化配置时表示URL及其缓存时间
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UrlWithTTL {
    /// Tracker列表源的URL
    #[serde(with = "crate::utils::url_serde")]
    url: url::Url,
    /// 可选的TTL(生存时间)，单位为秒
    /// None表示使用全局默认TTL
    ttl: Option<u64>,
}

/// 服务配置结构体
/// 定义了所有Tracker列表源的URL和对应的缓存策略
/// 使用#[serde(from/to)]自定义序列化行为，便于配置文件处理
#[derive(Debug, Clone, Serialize, Deserialize, Getset2)]
#[serde(into = "SerConfig")]
#[serde(from = "DeConfig")]
#[getset2(get_ref(pub))]
pub struct TrackerMergerConfig {
    /// 默认的缓存生存时间(Duration格式)
    ttl: Duration,
    /// Tracker列表源配置
    /// Key: Tracker列表源的URL
    /// Value: 可选的特定缓存时间，None表示使用全局ttl
    urls: HashMap<url::Url, Option<Duration>>,
}

impl Default for TrackerMergerConfig {
    /// 默认配置实现
    /// 预设三个常用的公开Tracker列表源，使用30秒默认缓存时间
    #[inline]
    fn default() -> Self {
        info!("正在初始化默认Tracker列表配置。");
        DeConfig::default().into()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, derive_builder::Builder)]
#[builder(
    private,
    name = "DeConfig",
    derive(Debug, serde::Deserialize),
    build_fn(
        private,
        error = std::convert::Infallible
    )
)]
struct SerConfig {
    /// TTL以秒为单位（便于配置文件读写）
    #[builder(default = 30)]
    ttl: u64,
    /// URL列表，包含TTL配置
    #[builder(
        default = [
            "https://fastly.jsdelivr.net/gh/ngosang/trackerslist/trackers_best_ip.txt",
            "https://fastly.jsdelivr.net/gh/ngosang/trackerslist/trackers_best.txt",
            "https://fastly.jsdelivr.net/gh/XIU2/TrackersListCollection/best.txt",
        ]
        .into_iter()
        .map(|url| {
            debug!("序列化Tracker源: {}", url);
            UrlWithTTL {
                url: url.parse::<url::Url>().expect("字面件构建失败。"),
                ttl: None,
            }
        })
        .collect()
    )]
    urls: Vec<UrlWithTTL>,
}

/// 用于序列化时将Duration转换为秒数
impl From<TrackerMergerConfig> for SerConfig {
    #[inline]
    fn from(value: TrackerMergerConfig) -> Self {
        let TrackerMergerConfig { urls, ttl } = value;
        info!("序列化Tracker列表配置，默认TTL: {}秒", ttl.as_secs());
        Self {
            // 将HashMap转换为Vec，并将Duration转换为秒数
            urls: urls
                .into_iter()
                .map(|(url, duration)| {
                    debug!("序列化Tracker源: {}", url);
                    UrlWithTTL {
                        url,
                        ttl: duration.map(|d| d.as_secs()), // Duration -> u64 seconds
                    }
                })
                .collect(),
            ttl: ttl.as_secs(), // Duration -> u64 seconds
        }
    }
}

impl From<SerConfig> for TrackerMergerConfig {
    #[inline]
    fn from(SerConfig { urls, ttl }: SerConfig) -> Self {
        info!("反序列化Tracker列表配置，默认TTL: {}秒", ttl);
        Self {
            // 将Vec转换为HashMap，并将秒数转换为Duration
            urls: urls
                .into_iter()
                .map(|UrlWithTTL { url, ttl }| {
                    debug!("反序列化Tracker源: {}", url);
                    (url, ttl.map(Duration::from_secs)) // u64 seconds -> Duration
                })
                .collect(),
            ttl: Duration::from_secs(ttl), // u64 seconds -> Duration
        }
    }
}

impl From<DeConfig> for TrackerMergerConfig {
    #[inline]
    fn from(config: DeConfig) -> Self {
        config.build().expect("unreachable.").into()
    }
}

fn build_summary_config_email_account(sc: &DeSummaryConfig) -> Option<EMailAccountConfig> {
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
        DeSummaryConfig::default().build().expect("unreachable.")
    }
}

impl From<DeSummaryConfig> for SummaryConfig {
    #[inline]
    fn from(de: DeSummaryConfig) -> Self {
        de.build().expect("unreachable.")
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
        s.serialize_str(addr.as_ref())
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

#[derive(Debug, Clone, Serialize, getset2::Getset2, derive_builder::Builder)]
#[getset2(get_ref(pub))]
#[builder(private, name = DeEMailAccountConfig, derive(Debug, serde::Deserialize), build_fn(private))]
pub struct EMailAccountConfig {
    #[builder_field_attr(serde(deserialize_with = "opt_url_deserialize"))]
    #[serde(with = "crate::utils::url_serde")]
    pub(super) server: url::Url,
    pub(super) uname: SensitiveString,
    pub(super) password: SensitiveString,
    #[builder(default)]
    pub(super) port: Option<u16>,
}

fn opt_url_deserialize<'de, D>(deserializer: D) -> Result<Option<url::Url>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(try_from = "Option<String>")]
    pub struct DeUrl(Option<url::Url>);
    impl TryFrom<Option<String>> for DeUrl {
        type Error = url::ParseError;

        #[inline]
        fn try_from(value: Option<String>) -> Result<Self, Self::Error> {
            if let Some(value) = value {
                Ok(Self(Some(url::Url::parse(&value)?)))
            } else {
                Ok(Self(None))
            }
        }
    }
    <DeUrl as serde::Deserialize>::deserialize(deserializer).map(|de_url| de_url.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use url::Url;

    // 辅助函数：创建一个简单的测试用Config
    fn test_config() -> TrackerMergerConfig {
        let mut urls = HashMap::new();
        urls.insert(
            Url::parse("https://example.com/trackers.txt").expect("字面件构建失败。"),
            Some(Duration::from_secs(120)),
        );
        urls.insert(
            Url::parse("https://example.org/list.txt").expect("字面件构建失败。"),
            None, // 使用全局ttl
        );
        TrackerMergerConfig {
            ttl: Duration::from_secs(60),
            urls,
        }
    }

    #[test]
    fn test_url_with_ttl_serde_roundtrip() {
        let original = UrlWithTTL {
            url: Url::parse("https://tracker.example.com/announce").expect("字面件构建失败。"),
            ttl: Some(300),
        };

        let toml_str = toml::to_string(&original).expect("序列化失败");
        let deserialized: UrlWithTTL = toml::from_str(&toml_str).expect("反序列化失败");

        assert_eq!(original.url, deserialized.url);
        assert_eq!(original.ttl, deserialized.ttl);
    }

    #[test]
    fn test_url_with_ttl_deserialize_without_ttl() {
        let toml_str = r#"url = "https://example.com/list.txt""#;
        let deserialized: UrlWithTTL = toml::from_str(toml_str).expect("反序列化失败");

        assert_eq!(deserialized.url.as_str(), "https://example.com/list.txt");
        assert!(deserialized.ttl.is_none());
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = test_config();

        // 序列化为 TOML
        let toml_str = toml::to_string_pretty(&config).expect("序列化失败");
        println!("序列化结果:\n{}", toml_str);

        // 反序列化回 Config
        let deserialized: TrackerMergerConfig = toml::from_str(&toml_str).expect("反序列化失败");

        // 验证字段
        assert_eq!(config.ttl, deserialized.ttl);
        assert_eq!(config.urls.len(), deserialized.urls.len());

        for (url, ttl) in &config.urls {
            let deser_ttl = deserialized.urls.get(url).expect("缺少预期的URL");
            assert_eq!(ttl, deser_ttl, "URL {} 的TTL不匹配", url);
        }
    }

    #[test]
    fn test_config_default_serde() {
        let config = TrackerMergerConfig::default();

        let toml_str = toml::to_string(&config).expect("默认配置序列化失败");
        let deserialized: TrackerMergerConfig =
            toml::from_str(&toml_str).expect("默认配置反序列化失败");

        assert_eq!(config.ttl, deserialized.ttl);
        assert_eq!(config.urls.len(), deserialized.urls.len());
    }

    #[test]
    fn test_config_deserialize_from_custom_format() {
        // 模拟手动编写的 TOML 配置
        let toml_str = r#"
ttl = 90

[[urls]]
url = "https://a.com/trackers.txt"
ttl = 30

[[urls]]
url = "https://b.com/best.txt"
"#;

        let config: TrackerMergerConfig = toml::from_str(toml_str).expect("反序列化失败");

        assert_eq!(config.ttl, Duration::from_secs(90));
        assert_eq!(config.urls.len(), 2);

        let url_a = Url::parse("https://a.com/trackers.txt").expect("字面件构建失败。");
        let url_b = Url::parse("https://b.com/best.txt").expect("字面件构建失败。");

        assert_eq!(config.urls[&url_a], Some(Duration::from_secs(30)));
        assert_eq!(config.urls[&url_b], None); // 未指定ttl，使用全局默认
    }

    #[test]
    fn test_config_deserialize_invalid_url_error() {
        let toml_str = r#"
ttl = 60

[[urls]]
url = "not a valid url"
ttl = 10
"#;

        let result: Result<TrackerMergerConfig, _> = toml::from_str(toml_str);
        assert!(result.is_err(), "反序列化无效URL应当失败");
    }

    #[test]
    fn test_config_conversion_identity() {
        let config = test_config();
        let serde_config: SerConfig = config.clone().into();
        let config_back: TrackerMergerConfig = serde_config.into();

        assert_eq!(config.ttl, config_back.ttl);
        assert_eq!(config.urls.len(), config_back.urls.len());
        for (url, ttl) in &config.urls {
            assert_eq!(ttl, config_back.urls.get(url).expect("字面件构建失败。"));
        }
    }

    #[test]
    fn test_empty_config_serde() {
        let config = TrackerMergerConfig {
            ttl: Duration::from_secs(10),
            urls: HashMap::new(),
        };

        let toml_str = toml::to_string(&config).expect("序列化失败");
        let deserialized: TrackerMergerConfig = toml::from_str(&toml_str).expect("反序列化失败");

        assert!(deserialized.urls.is_empty());
        assert_eq!(deserialized.ttl, Duration::from_secs(10));
    }
}
