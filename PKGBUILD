# Maintainer: Jason Wang <wang_borong@163.com>
pkgname=omnidoc
pkgver=1.7.0
pkgrel=1
pkgdesc="OmniDoc - A documentation tool"
arch=('x86_64')
url="https://github.com/wang-borong/omnidoc"
license=('MIT')
depends=(
  'brotli'
  'ca-certificates'
  'glibc'
  'graphite'
  'libgcc'
  'libstdc++'
  'openssl'
  'pandoc'
  'pandoc-crossref-bin'
  'noto-fonts-cjk'
  'zlib'
  'zstd'
)
optdepends=(
  'texlive-basic: XeLaTeX/latexmk compatibility for advanced raw LaTeX projects'
  'texlive-binextra: latexmk and additional TeX utilities'
  'biber: biblatex/Biber projects'
  'texlive-fontsextra: Additional fonts'
  'drawio-desktop: Draw.io support'
  'graphviz: Graph visualization'
  'plantuml: PlantUML diagrams'
  'inkscape: SVG editing'
  'imagemagick: Image manipulation'
)
source=("${pkgname}-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz::https://github.com/wang-borong/omnidoc/releases/download/v${pkgver}/${pkgname}-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz")
sha256sums=('SKIP')

package() {
  cd "${srcdir}/${pkgname}-v${pkgver}-x86_64-unknown-linux-gnu"
  install -Dm755 omnidoc "${pkgdir}/usr/bin/omnidoc"
  install -Dm755 engines/tectonic "${pkgdir}/usr/lib/omnidoc/tectonic"
  install -Dm644 README.md "${pkgdir}/usr/share/doc/omnidoc/README.md"
  install -Dm644 CHANGELOG.md "${pkgdir}/usr/share/doc/omnidoc/CHANGELOG.md"
  install -Dm644 THIRD_PARTY_LICENSES.md \
    "${pkgdir}/usr/share/doc/omnidoc/THIRD_PARTY_LICENSES.md"
  install -Dm644 docs/decisions/0001-tectonic-engine-policy.md \
    "${pkgdir}/usr/share/doc/omnidoc/docs/decisions/0001-tectonic-engine-policy.md"
}
