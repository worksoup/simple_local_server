# Maintainer: learturely <learturely@gmail.com>
# Contributor: learturely <learturely@gmail.com>

pkgname=simple-local-server
_reponame=simple_local_server
_binname=simple_local_server
pkgver=0.1.0
pkgrel=1
pkgdesc="Simple local server: Tieba sign daemon & tracker merger"
arch=('x86_64' 'aarch64')
url="https://github.com/worksoup/${_reponame}"
license=('MIT')
depends=('gcc-libs' 'openssl')
makedepends=('rust' 'cargo' 'git')
source=("git+${url}.git")
sha256sums=('SKIP')

build() {
  cd "$srcdir/$_reponame"
  cargo build --release
}

package() {
  cd "$srcdir/$_reponame"

  # ---------- 自定义消息 ----------
  echo ":: 正在安装 ${pkgname}，配置文件将放在 /var/lib/sl-server/"
  echo ":: 启动服务前请编辑 /var/lib/sl-server/sl-server.toml"
  echo ":: 查看日志: journalctl -u ${pkgname} -f"
  # --------------------------------

  # 安装二进制文件
  install -Dm755 "target/release/${_binname}" "${pkgdir}/usr/bin/${_binname}"
  
  # 安装许可证文件
  install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"

  # 安装只读的参考配置到 /etc
  install -Dm644 "sl-server.toml" "${pkgdir}/etc/sl-server/sl-server.toml.example"

  # 生成并安装 systemd 服务文件
  sed -e "s|ExecStart=.*|ExecStart=/usr/bin/${_binname} -c /var/lib/sl-server/sl-server.toml|" \
      -e "s|ReadWritePaths=.*|ReadWritePaths=/var/lib/sl-server|" \
      "systemd-units/sl-server.service" |
      install -Dm644 /dev/stdin "${pkgdir}/usr/lib/systemd/system/${pkgname}.service"
}
