# 一些本地服务

## 配置

```toml
addr = "127.0.0.1:5139"
log_dir = "/var/lib/sl-server/logs/"

[tracker_list_config]
ttl = 30

[[tracker_list_config.urls]]
url = "https://fastly.jsdelivr.net/gh/ngosang/trackerslist/trackers_best.txt"
ttl = 3600

[[tracker_list_config.urls]]
url = "https://fastly.jsdelivr.net/gh/ngosang/trackerslist/trackers_best_ip.txt"
ttl = 3600

[[tracker_list_config.urls]]
url = "https://fastly.jsdelivr.net/gh/XIU2/TrackersListCollection/best.txt"
ttl = 3600

[email_account]
server = "smtp://smtp.url.to.server"
uname = "uname@url.to.server"
password = "<PASSWD>"

[tieba_sign_config]
bduss = "BDUSS in Cookies"
dont_ntfy = []
sign_result_send_to = "rx@rx.com"
```

修改配置后需重启服务。

## 功能

### tracker 合并

GET 请求 `addr/tracker_list`, 服务将返回 `tracker_list_config.urls` 中所有 tracker 合并成的列表。

### 贴吧自动签到

配置 `tieba_sign_config.bduss` 后，程序启动时及每天自动签到，且每 15 分钟检查是否有新关注吧并签到。支持电邮通知，需按如上示例配置。

理论上可以用 github action 自动运行，不过目前没有专门适配。

## PKG

使用 `cargo deb` 可以生成 deb 包。

`PKGBUILD` 供 Arch Linux 用户使用。

生成的包带有 systemd 服务，服务运行后会将示例配置文件复制到 `/var/lib/sl-server/sl-server.toml` 作为配置文件。
修改该配置后需重启服务。

## LICENSE

见 [LICENSE](./LICENSE).
