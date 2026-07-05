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

use std::{fmt::Display, num::NonZeroUsize, sync::Arc};

use reqwest::{
    Client, IntoUrl, Method, RequestBuilder, Url,
    header::{HeaderValue, USER_AGENT},
};
use serde::Deserialize;

use crate::utils::{SensitiveStr, SensitiveString};
use crate::{config::TiebaSignConfig, mailer::EMailer};

#[inline(always)]
pub fn percent_encode(input: &'_ str) -> impl Display + Iterator<Item = &'_ str> {
    percent_encoding::utf8_percent_encode(input, percent_encoding::NON_ALPHANUMERIC)
}
#[inline(always)]
pub fn md5_hash<T: AsRef<[u8]>>(input: T) -> [u8; 16] {
    md5::compute(input).0
}

pub trait WithUserAgent: Sized {
    /// Add a `Header` to this Request.
    fn with_user_agent<UserAgent>(self, user_agent: UserAgent) -> RequestBuilder
    where
        HeaderValue: TryFrom<UserAgent>,
        <HeaderValue as TryFrom<UserAgent>>::Error: Into<http::Error>;
}

impl WithUserAgent for RequestBuilder {
    #[inline(always)]
    fn with_user_agent<UserAgent>(self, user_agent: UserAgent) -> RequestBuilder
    where
        HeaderValue: TryFrom<UserAgent>,
        <HeaderValue as TryFrom<UserAgent>>::Error: Into<http::Error>,
    {
        self.header(USER_AGENT, user_agent)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginInfo {
    #[serde(rename = "tbs")]
    tbs: SensitiveString,
    #[serde(rename = "is_login")]
    is_login: i64,
}

impl LoginInfo {
    #[inline(always)]
    pub fn tbs(&'_ self) -> SensitiveStr<'_> {
        self.tbs.as_ref()
    }
    #[inline(always)]
    pub fn is_login(&self) -> bool {
        self.is_login == 1
    }
}

#[derive(Debug, Clone)]
pub struct Tieba {
    id: i64,
    name: String,
    signed: bool,
    liked: bool,
}

#[derive(Debug, Clone)]
pub struct LikedTiebaList(Vec<Tieba>);

#[derive(Debug)]
pub enum SignResult {
    Ok {
        rank: Option<usize>,
        score: u32,
    },
    Signed,
    Invalid,
    Unfollowed,
    Failure {
        code: Option<i64>,
        msg: Option<String>,
    },
    Error(Error),
}

impl Tieba {
    const SIGN_KEY: &str = "tiebaclient!!!";
    #[inline(always)]
    pub fn new(id: i64, name: String, signed: bool, liked: bool) -> Self {
        Self {
            id,
            name,
            signed,
            liked,
        }
    }

    #[inline(always)]
    pub fn name(&self) -> &String {
        &self.name
    }

    #[inline(always)]
    pub fn signed(&self) -> bool {
        self.signed
    }

    #[inline(always)]
    pub fn liked(&self) -> bool {
        self.liked
    }

    #[inline(always)]
    pub fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    #[inline(always)]
    pub fn signed_mut(&mut self) -> &mut bool {
        &mut self.signed
    }

    #[inline(always)]
    pub fn liked_mut(&mut self) -> &mut bool {
        &mut self.liked
    }

    #[tracing::instrument(skip(session))]
    pub async fn sign_mobile(&self, session: &Session, tbs: SensitiveStr<'_>) -> SignResult {
        let bduss = session.config.bduss().0.as_str();
        let id = self.id.to_string();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        let mut params = vec![
            ("_client_type", "2"),
            ("_client_version", "9.7.8.0"),
            ("_phone_imei", "000000000000000"),
            ("model", "mobile"),
            ("net_type", "1"),
            ("BDUSS", bduss),
            ("fid", id.as_str()),
            ("kw", self.name.as_str()),
            ("tbs", tbs.0),
            ("timestamp", timestamp.as_str()),
        ];
        params.sort_by_key(|(k, _v)| *k);
        let sign_params = params
            .iter()
            .map(|(k, v)| format!("{}={}", *k, *v))
            .collect::<Vec<_>>();
        let sign = hex::encode(md5::compute(sign_params.join("") + Self::SIGN_KEY).0);
        params.push(("sign", &sign));
        #[derive(Debug, Clone, Deserialize)]
        pub struct UserInfo {
            #[serde(rename = "user_sign_rank")]
            user_sign_rank: Option<String>,
            #[serde(rename = "is_sign_in")]
            is_sign_in: Option<String>,
            #[serde(rename = "sign_bonus_point")]
            sign_bonus_point: Option<String>,
        }
        #[derive(Debug, Clone, Deserialize)]
        pub struct SignResponse {
            #[serde(rename = "user_info")]
            user_info: Option<UserInfo>,
            #[serde(rename = "error_code")]
            error_code: String,
            #[serde(rename = "error_msg")]
            error_msg: Option<String>,
        }
        let resp = match session.post(Session::SIGN_URL).form(&params).send().await {
            Ok(resp) => resp,
            Err(e) => return SignResult::Error(Error::from(e)),
        };
        let SignResponse {
            user_info,
            error_code,
            error_msg,
        } = resp.json().await.expect("failed parsing sign result.");
        match error_code.as_str() {
            "0" if let Some(user_info) = user_info
                && user_info.is_sign_in.as_ref().is_some_and(|s| s == "1") =>
            {
                let score = user_info
                    .sign_bonus_point
                    .as_ref()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(6);
                SignResult::Ok {
                    rank: user_info
                        .user_sign_rank
                        .as_ref()
                        .and_then(|s| s.parse().ok()),
                    score,
                }
            }
            "160002" => SignResult::Signed,
            "340006" => SignResult::Invalid,
            "340004" => SignResult::Unfollowed,
            "0" => SignResult::Failure {
                code: Some(0),
                msg: error_msg.or_else(|| Some("未知原因签到失败。".to_owned())),
            },
            _ => SignResult::Failure {
                code: error_code.parse().ok(),
                msg: error_msg,
            },
        }
    }
}
#[derive(Debug, Clone)]
pub struct LikedTiebaListPN {
    list: Vec<Tieba>,
    next_page: Option<NonZeroUsize>,
}

impl LikedTiebaListPN {
    #[inline(always)]
    pub const fn new(list: Vec<Tieba>, next_page: Option<NonZeroUsize>) -> Self {
        Self { list, next_page }
    }
    #[inline(always)]
    pub const fn list(&self) -> &Vec<Tieba> {
        &self.list
    }

    #[inline(always)]
    pub const fn next_page(&self) -> Option<NonZeroUsize> {
        self.next_page
    }

    #[inline(always)]
    pub fn list_mut(&mut self) -> &mut Vec<Tieba> {
        &mut self.list
    }

    #[inline(always)]
    pub fn next_page_mut(&mut self) -> &mut Option<NonZeroUsize> {
        &mut self.next_page
    }
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    ReqwestErr(#[from] reqwest::Error),
    #[error("{err_msg}")]
    BadRequest { err_msg: String },
    #[error("登入失败")]
    LoginFailed,
    #[error("任务已取消")]
    TaskCanceled,
    #[error(transparent)]
    MailerError(#[from] lettre::transport::smtp::Error),
}
#[derive(Debug, Clone)]
pub struct Session {
    client: Client,
    config: TiebaSignConfig,
    cookies: Arc<reqwest_cookie_store::CookieStoreMutex>,
}

impl Session {
    #[inline(always)]
    pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.client.request(Method::GET, url)
    }
    #[inline(always)]
    pub fn post<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::POST, url)
    }
    #[inline(always)]
    pub fn put<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PUT, url)
    }
    #[inline(always)]
    pub fn patch<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PATCH, url)
    }
    #[inline(always)]
    pub fn delete<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }
    #[inline(always)]
    pub fn head<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::HEAD, url)
    }
    #[inline(always)]
    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        self.client.request(method, url)
    }
    #[inline(always)]
    pub fn execute(
        &self,
        request: reqwest::Request,
    ) -> impl Future<Output = Result<reqwest::Response, reqwest::Error>> {
        self.client.execute(request)
    }
}

impl Session {
    const BAIDU_DOMAIN: &str = "http://baidu.com/";
    const LIKE_URL: &str = "https://tieba.baidu.com/mo/q/newmoindex";
    const TBS_URL: &str = "http://tieba.baidu.com/dc/common/tbs";
    const SIGN_URL: &str = "http://c.tieba.baidu.com/c/c/forum/sign";
    const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/39.0.2171.71 Safari/537.36";
    const I_TIEBA: &str = "https://tieba.baidu.com/i/i/forum";
    const HOME_SIDEBAR_LEFT: &str = "https://tieba.baidu.com/c/f/pc/homeSidebarLeft";

    #[tracing::instrument]
    pub fn new(config: TiebaSignConfig) -> Result<Self, reqwest::Error> {
        let mut cookie_store = reqwest_cookie_store::CookieStore::default();
        Self::set_bduss(&config, &mut cookie_store);
        let cookie_store = reqwest_cookie_store::CookieStoreMutex::new(cookie_store);
        let cookies = Arc::new(cookie_store);
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                // return attempt.stop();
                if attempt.previous().len() > 10 {
                    attempt.stop()
                } else if let Some(url) = attempt.previous().last() {
                    tracing::info!("{}/{}: {url}", attempt.previous().len(), attempt.status());
                    attempt.follow()
                } else {
                    attempt.follow()
                }
            }))
            .cookie_provider(Arc::clone(&cookies))
            .build()?;
        Ok(Self {
            client,
            cookies,
            config,
        })
    }
    #[inline]
    fn set_bduss(config: &TiebaSignConfig, cookie_store: &mut reqwest_cookie_store::CookieStore) {
        let cookie = reqwest_cookie_store::RawCookie::build(("BDUSS", config.bduss().0.as_str()))
            .path("/")
            .domain(".baidu.com")
            .build();
        cookie_store
            .insert_raw(&cookie, &Url::parse(Self::BAIDU_DOMAIN).unwrap())
            .unwrap();
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_liked_tieba_list_pn(
        &self,
        n: NonZeroUsize,
    ) -> Result<LikedTiebaListPN, Error> {
        let sign = hex::encode(md5_hash(format!(
            "_client_type=20list_type=likepn={n}subapp_type=pc36770b1f34c9bbf2e7d1a99d2b82fa9e",
        )));
        let url = format!(
            "{}?pn={n}&list_type=like&subapp_type=pc&_client_type=20&sign={sign}",
            Self::HOME_SIDEBAR_LEFT
        );
        #[derive(Debug, Clone, Deserialize)]
        pub struct TiebaRaw {
            #[serde(rename = "id")]
            id: i64,
            #[serde(rename = "name")]
            name: String,
            #[serde(rename = "is_signed")]
            is_signed: i32,
            #[serde(rename = "level_id")]
            level: u32,
        }
        #[derive(Debug, Clone, Deserialize)]
        pub struct TiebaListRaw {
            #[serde(rename = "like_forum_list")]
            like_forum_list: Option<Vec<TiebaRaw>>,
            #[serde(rename = "like_forum_has_more")]
            like_forum_has_more: i32,
        }
        #[derive(Debug, Clone, Deserialize)]
        pub struct TiebaListResponse {
            #[serde(rename = "error_code")]
            error_code: i32,
            #[serde(rename = "data")]
            data: Option<TiebaListRaw>,
        }
        let resp = self
            .get(url)
            .send()
            .await?
            .json::<TiebaListResponse>()
            .await?;
        let Some(data) = resp.data else {
            return Err(if resp.error_code != 0 {
                Error::BadRequest {
                    err_msg: format!(
                        "获取已关注贴吧列表第 {n} 页失败，错误码：{error_code}.",
                        error_code = resp.error_code
                    ),
                }
            } else {
                Error::BadRequest {
                    err_msg: format!("获取已关注贴吧列表第 {n} 页失败，无数据。"),
                }
            });
        };
        let Some(list) = data.like_forum_list else {
            return Err(Error::BadRequest {
                err_msg: format!("获取已关注贴吧列表第 {n} 页失败，无数据。"),
            });
        };
        Ok(LikedTiebaListPN::new(
            list.into_iter()
                .map(|raw| {
                    let TiebaRaw {
                        id,
                        name,
                        is_signed,
                        level: _,
                    } = raw;
                    Tieba::new(id, name, is_signed != 0, true)
                })
                .collect(),
            (data.like_forum_has_more != 0).then_some(n.saturating_add(1)),
        ))
    }
    #[tracing::instrument(skip(self), fields(config = ?self.config), err)]
    pub async fn get_all_liked_tieba(&self) -> Result<LikedTiebaList, Error> {
        let mut next_page = Some(const { NonZeroUsize::new(1).unwrap() });
        let mut likes = Vec::new();
        while let Some(page) = next_page {
            if let Ok(mut list) = self.get_liked_tieba_list_pn(page).await {
                next_page = list.next_page();
                likes.append(list.list_mut());
            } else {
                next_page = None;
            }
            // 0.5--1.5s
            random_delay(500..=1500).await;
        }
        Ok(LikedTiebaList(likes))
    }

    #[tracing::instrument(skip(self), fields(config = ?self.config), err)]
    pub async fn refresh_login_info(&self) -> Result<LoginInfo, reqwest::Error> {
        tracing::debug!("请求 TBS 接口");
        let r = self
            .get(Self::TBS_URL)
            .with_user_agent(Self::USER_AGENT)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("charset", "UTF-8")
            .build()?;
        for (name, value) in r.headers() {
            tracing::debug!(
                "req - header: `{name}`: `{:?}`",
                str::from_utf8(value.as_bytes())
            );
        }
        let r = self.execute(r).await?;
        for (name, value) in r.headers() {
            tracing::debug!(
                "res - header: `{name}`: `{:?}`",
                str::from_utf8(value.as_bytes())
            );
        }
        Ok(r.json().await.expect("failed to get tbs response"))
    }
    #[tracing::instrument(skip(self), fields(config = ?self.config))]
    pub async fn sign_liked_tieba(
        &self,
        liked_tieba: LikedTiebaList,
        login_info: &LoginInfo,
    ) -> (LikedTiebaList, Vec<SignResult>, Vec<(Tieba, SignResult)>) {
        let mut rest = Vec::new();
        let mut rest_tieba_sign_result = Vec::new();
        let mut this_turn_not_to_sign = Vec::new();
        for tieba in liked_tieba.0.into_iter().filter(|f| !f.signed) {
            let result = tieba.sign_mobile(&self, login_info.tbs()).await;
            match result {
                SignResult::Ok { rank, score } => {
                    if let Some(rank) = rank {
                        tracing::warn!("第 {rank} 个在{}吧签到，经验加 {score}.", tieba.name())
                    } else {
                        tracing::warn!("在{}吧签到，经验加 {score}.", tieba.name())
                    }
                }
                SignResult::Signed => {
                    tracing::warn!("已签过{}吧。", tieba.name())
                }
                SignResult::Invalid => {
                    tracing::warn!("{}吧无效，签到失败！", tieba.name());
                    this_turn_not_to_sign.push((tieba, SignResult::Invalid));
                }
                SignResult::Failure { code, msg } => {
                    let common_msg = format_args!("{}吧签到失败", tieba.name());
                    if let Some(msg) = msg.as_ref() {
                        if let Some(code) = code {
                            tracing::warn!("{common_msg}：`{msg}`.错误码为 `{code}`.")
                        } else {
                            tracing::warn!("{common_msg}：`{msg}`.");
                        };
                    } else {
                        if let Some(code) = code {
                            tracing::warn!("{common_msg}！错误码为 `{code}`.")
                        } else {
                            tracing::warn!("{common_msg}！");
                        };
                    };
                    rest.push(tieba);
                    rest_tieba_sign_result.push(SignResult::Failure { code, msg });
                }
                SignResult::Unfollowed => {
                    tracing::warn!("未关注{}吧，签到失败！", tieba.name());
                    this_turn_not_to_sign.push((tieba, SignResult::Unfollowed));
                }
                SignResult::Error(e) => {
                    tracing::warn!("{}吧签到失败：`{e}`.", tieba.name());
                    rest.push(tieba);
                }
            }
            // 1--2.5s
            random_delay(1000..=2500).await;
        }
        (
            LikedTiebaList(rest),
            rest_tieba_sign_result,
            this_turn_not_to_sign,
        )
    }
}

#[tracing::instrument]
fn get_new_session() -> Session {
    let bduss = std::env::var("BDUSS").unwrap();
    let config = TiebaSignConfig::new(bduss.into(), vec![], None);
    Session::new(config).unwrap()
}

#[tracing::instrument]
pub async fn random_delay<R>(range: R)
where
    R: rand::distr::uniform::SampleRange<u64> + std::fmt::Debug,
{
    tracing::info!("随机延时……");
    use tokio::time::{Duration, sleep};
    let millis = rand::random_range(range);
    sleep(Duration::from_millis(millis)).await;
}

#[derive(Debug)]
pub struct TiebaSignDaemon {
    task: tokio::task::JoinHandle<()>,
    cancellation: tokio_util::sync::CancellationToken,
}

impl TiebaSignDaemon {
    #[tracing::instrument(skip(self))]
    pub async fn stop(self) {
        self.cancellation.cancel();
        self.task.await.unwrap()
    }
    #[tracing::instrument(skip_all, fields(sign_result_send_to = ?config.sign_result_send_to()))]
    pub async fn run(
        config: TiebaSignConfig,
        emailer: actix_web::web::Data<Option<EMailer>>,
    ) -> Result<Self, Error> {
        let token = tokio_util::sync::CancellationToken::new();
        let cloned_token = token.clone();
        let join_handle = tokio::spawn(async move {
            tokio::select! {
                e = sign_daemon(config, (&**emailer).as_ref()) => {
                    let e = e.unwrap_err();
                    tracing::error!("贴吧签到服务运行失败：`{e}`.");
                }
                _ = cloned_token.cancelled() => {
                    tracing::error!("贴吧签到服务运行已停止。");
                }
            }
        });
        Ok(Self {
            task: join_handle,
            cancellation: token,
        })
    }
}

#[tracing::instrument(skip_all)]
async fn sign_daemon(
    config: TiebaSignConfig,
    mailer: Option<&EMailer>,
) -> Result<std::convert::Infallible, Error> {
    use std::{
        collections::HashSet,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    let session = Session::new(config.clone())?;
    let mut login_info = session.refresh_login_info().await?;
    if !login_info.is_login() {
        return Err(Error::LoginFailed);
    }

    // 上次的签到列表。
    let mut last_liked_ids: HashSet<i64> = HashSet::new();
    // 上次签到时间（日）。
    let mut last_sign_day: Option<u64> = None;
    // 用于 5 分钟刷新登入信息的计时器。
    let mut last_login_info_refresh = current_timestamp_secs();

    /// 辅助函数：检查并刷新登入信息。
    /// 如果出错返回 `Err(())`, 否则刷新返回 `Ok(true)`, 未刷新返回 `Ok(false)`.
    #[tracing::instrument]
    async fn check_and_refresh_login_info(
        session: &Session,
        login_info: &mut LoginInfo,
        last_refresh: &mut u64,
    ) -> Result<bool, ()> {
        let now = current_timestamp_secs();
        if now - *last_refresh > 300 {
            tracing::debug!("刷新登录信息……");
            match session.refresh_login_info().await {
                Ok(li) => {
                    *login_info = li;
                    *last_refresh = current_timestamp_secs();
                    Ok(true)
                }
                Err(e) => {
                    tracing::error!("刷新登入信息失败: `{e}`.");
                    Err(())
                }
            }
        } else {
            Ok(false)
        }
    }

    fn current_timestamp_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn day_of_timestamp(secs: u64) -> u64 {
        secs / (24 * 60 * 60)
    }

    loop {
        // 1. 获取全部喜欢的贴吧
        'r#continue: {
            let all_likes = match session.get_all_liked_tieba().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("获取已关注贴吧列表失败: `{e}`.");
                    break 'r#continue;
                }
            };
            if check_and_refresh_login_info(&session, &mut login_info, &mut last_login_info_refresh)
                .await
                .is_err()
            {
                break 'r#continue;
            };

            let today = day_of_timestamp(current_timestamp_secs());

            // 2. 确定本轮需要签到的贴吧
            let to_sign = if last_sign_day != Some(today) {
                // 新的一天或首次运行，全量签到
                tracing::info!("新的一天 (day {today})，开始签到所有贴吧。");
                last_liked_ids.clear();
                all_likes.clone()
            } else {
                let new_likes: Vec<Tieba> = all_likes
                    .0
                    .iter()
                    .filter(|t| !last_liked_ids.contains(&t.id))
                    .cloned()
                    .collect();
                LikedTiebaList(new_likes)
            };

            // 3. 签到主流程
            if !to_sign.0.is_empty() {
                let (mut rest, mut rest_tieba_sign_result, mut this_turn_not_to_sign) =
                    session.sign_liked_tieba(to_sign, &login_info).await;
                if check_and_refresh_login_info(
                    &session,
                    &mut login_info,
                    &mut last_login_info_refresh,
                )
                .await
                .is_err()
                {
                    break 'r#continue;
                };

                let mut flag1 = 3;
                let mut flag2 = 5;
                let mut freshed = false;
                while flag1 > 0 || (flag2 > 0 && freshed) {
                    if rest.0.is_empty() {
                        break;
                    }
                    tracing::warn!("{} 个吧未完成签到，重试...", rest.0.len());
                    let (rest_, rest_tieba_sign_result_, mut this_turn_not_to_sign_) =
                        session.sign_liked_tieba(rest, &login_info).await;
                    rest = rest_;
                    rest_tieba_sign_result = rest_tieba_sign_result_;
                    this_turn_not_to_sign.append(&mut this_turn_not_to_sign_);
                    if let Ok(f) = check_and_refresh_login_info(
                        &session,
                        &mut login_info,
                        &mut last_login_info_refresh,
                    )
                    .await
                    {
                        freshed = f;
                        if f {
                            flag2 -= 1;
                        } else {
                            flag1 -= 1;
                        }
                    } else {
                        flag1 -= 1;
                        flag2 -= 1;
                    }
                }
                if !rest.0.is_empty() {
                    tracing::warn!(
                        "以下 {} 个吧最终未能签到: {:?}",
                        rest.0.len(),
                        rest.0.iter().map(|t| &t.name).collect::<Vec<_>>()
                    );
                }
                match (mailer, session.config.sign_result_send_to().as_ref()) {
                    (Some(mailer), Some(send_to)) => {
                        let mut failure_msgs = Vec::new();
                        for (tieba, result) in rest
                            .0
                            .into_iter()
                            .zip(rest_tieba_sign_result)
                            .chain(this_turn_not_to_sign.into_iter())
                        {
                            if config.dont_ntfy().contains(tieba.name()) {
                                continue;
                            }
                            let reason = match &result {
                                SignResult::Invalid => "贴吧无效".to_owned(),
                                SignResult::Unfollowed => "未关注该吧".to_owned(),
                                SignResult::Failure { code, msg } => {
                                    let mut s =
                                        msg.clone().unwrap_or_else(|| "未知原因".to_owned());
                                    if let Some(c) = code {
                                        s.push_str(&format!(" (错误码: {c})"));
                                    }
                                    s
                                }
                                SignResult::Error(e) => e.to_string(),
                                _ => "未知错误（可能是误报）".to_owned(),
                            };
                            failure_msgs.push((tieba.name().to_owned(), reason));
                        }
                        if !failure_msgs.is_empty() {
                            match build_failure_notification_email(
                                &failure_msgs,
                                &mailer.sender_addr(),
                                &send_to,
                            ) {
                                Ok(message) => {
                                    if let Err(e) = mailer.send(&message) {
                                        tracing::error!("发送签到失败邮件出错: `{e}`.");
                                    }
                                }
                                Err(e) => tracing::error!("构建失败通知邮件出错: `{e}`."),
                            }
                        }
                    }
                    (None, Some(_)) => {
                        tracing::warn!("消息接收方已定义，但未配置 Mailer.")
                    }
                    (Some(_), None) => {
                        tracing::warn!("已配置 Mailer, 但消息接收方未定义。")
                    }
                    (None, None) => {
                        tracing::warn!(
                            "未填写消息发送相关配置，请配置 Mailer 并在贴吧签到配置中定义消息接收方。"
                        )
                    }
                }
            } else {
                tracing::debug!("本轮没有需要签到的贴吧。");
            }

            // 4. 更新状态
            last_liked_ids.extend(all_likes.0.iter().map(|t| t.id));
            last_sign_day = Some(today);
        }
        // 5. 下次循环前等待 15 分。
        tracing::debug!("等待 15 分后进入下一轮...");
        tokio::time::sleep(Duration::from_mins(15)).await;
    }
}

/// 构造签到失败通知邮件
///
/// * `failure_msgs` - 失败条目列表，每个元素为 (贴吧名称, 失败原因)
/// * `from_addr`     - 发件人邮箱地址
/// * `to_addr`       - 收件人邮箱地址
#[tracing::instrument]
fn build_failure_notification_email(
    failure_msgs: &[(String, String)],
    from_addr: &lettre::Address,
    to_addr: &lettre::Address,
) -> Result<lettre::Message, lettre::error::Error> {
    use lettre::message::{Mailbox, MultiPart, SinglePart};
    // 纯文本版本：带序号的列表
    let plain_body = failure_msgs
        .iter()
        .enumerate()
        .map(|(i, (name, reason))| format!("{}. {} — {}", i + 1, name, reason))
        .collect::<Vec<_>>()
        .join("\n");

    // HTML 版本：表格，贴吧名称超链接到其主页
    let mut html_body = String::from(
        "<table border='1' cellpadding='5' cellspacing='0' style='border-collapse:collapse;'>\
         <tr><th>贴吧名称</th><th>失败原因</th></tr>",
    );
    for (name, reason) in failure_msgs {
        let encoded_name: String = percent_encode(name).collect();
        let url = format!("https://tieba.baidu.com/f?kw={encoded_name}");
        html_body.push_str(&format!(
            "<tr><td><a href=\"{url}\">{name}</a></td><td>{reason}</td></tr>"
        ));
    }
    html_body.push_str("</table>");

    lettre::Message::builder()
        .from(Mailbox::new(
            "贴吧签到助手".to_owned().into(),
            from_addr.clone(),
        ))
        .to(Mailbox::new("管理员".to_owned().into(), to_addr.clone()))
        .subject("贴吧签到失败通知")
        .multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::plain(plain_body))
                .singlepart(SinglePart::html(html_body)),
        )
}
#[cfg(test)]
mod tests {
    use crate::tieba_sign::{Tieba, get_new_session, md5_hash};
    use std::num::NonZeroUsize;

    #[test]
    fn test_md5_hash() {
        let result = hex::encode(md5_hash("test_md5_hash"));
        assert_eq!(result, "5373be58df1c9d99a0cb186d6e5596ff");
        let result = hex::encode(md5_hash(
            "_client_type=20amis_key=6e49499d1b4d0ce92d5ad4398c5de91f12subapp_type=pc36770b1f34c9bbf2e7d1a99d2b82fa9e",
        ));
        println!("{result}");
        assert_eq!(result, "93bb3e8f5547b56c220f210a8cd23f82");
        let result = hex::encode(md5_hash(
            "_client_type=20subapp_type=pc36770b1f34c9bbf2e7d1a99d2b82fa9e",
        ));
        println!("{result}");
        assert_eq!(result, "e9b101df871c39eedcf9a232c2d26ec8");
        let result = hex::encode(md5_hash(
            "_client_type=20kw=%E9%B8%A1subapp_type=pctbs=b213e16f2a859a0d178309093436770b1f34c9bbf2e7d1a99d2b82fa9e",
        ));
        println!("{result}");
        assert_eq!(result, "9da8315c36bc25b04b0f3b67f9bff4ec");
        let result = hex::encode(md5_hash(
            "_client_type=20is_newfeed=1is_newfrs=1kw=%E9%B8%A1pn=1rn=30rn_need=10sort_type=-1subapp_type=pctbs=b213e16f2a859a0d178309093436770b1f34c9bbf2e7d1a99d2b82fa9e",
        ));
        println!("{result}");
        assert_eq!(result, "ef449713431cae0aa336179ffc565447");
        let result = hex::encode(md5_hash(
            "_client_type=20forum_name=鸡subapp_type=pc36770b1f34c9bbf2e7d1a99d2b82fa9e",
        ));
        println!("{result}");
        assert_eq!(result, "c89ed343aa50b0f75608ce7b24cdd69e");
        let result = hex::encode(md5_hash(
            "_client_type=20list_type=likepn=2subapp_type=pc36770b1f34c9bbf2e7d1a99d2b82fa9e",
        ));
        println!("{result}");
        assert_eq!(result, "65230631eb09d6f9bf9f8f730082ff7e");
    }

    #[tokio::test]
    async fn test_get_tbs() {
        crate::test_utils::read_dotenv();
        crate::test_utils::init_subscriber();
        let session = get_new_session();
        let tbs = session.refresh_login_info().await.unwrap();
        tracing::info!("{tbs:?}");
        // assert!(tbs.is_login());
        for cookie in session.cookies.lock().unwrap().iter_any() {
            tracing::info!("{cookie:?}");
        }
    }

    #[tokio::test]
    async fn test_get_liked_tieba_pn() {
        crate::test_utils::read_dotenv();
        crate::test_utils::init_subscriber();
        let session = get_new_session();
        let tbs = session.refresh_login_info().await.unwrap();
        assert!(tbs.is_login());
        let mut next_page = Some(NonZeroUsize::new(1).unwrap());
        let mut likes = Vec::new();
        while let Some(page) = next_page {
            if let Ok(mut list) = session.get_liked_tieba_list_pn(page).await {
                next_page = list.next_page();
                likes.append(list.list_mut());
            } else {
                next_page = None;
            }
        }
        tracing::info!("{:#?}", likes);
        tracing::info!("{}", likes.len());
        let tieba = likes.iter().rfind(|s| s.id == 2633848).unwrap();
        let result = tieba.sign_mobile(&session, tbs.tbs()).await;
        tracing::info!("{}吧签到结果：{result:?}", tieba.name.as_str());
        let unfollowed_tieba = Tieba::new(27935890, "meme图".to_owned(), false, false);
        let result = unfollowed_tieba.sign_mobile(&session, tbs.tbs()).await;
        tracing::info!("{}吧签到结果：{result:?}", unfollowed_tieba.name.as_str());
    }
}
