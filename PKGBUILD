# Maintainer: Jason Wang <wang_borong@163.com>
pkgname=omnidoc
pkgver=1.0.0
pkgrel=1
pkgdesc="OmniDoc - A documentation tool"
arch=('x86_64')
url="https://github.com/wang-borong/omnidoc"
license=('MIT')
depends=(
  'pandoc'
  'pandoc-crossref-bin'
  'texlive-basic'
  'texlive-bibtexextra'
  'texlive-bin'
  'texlive-fontsrecommended'
  'texlive-langchinese'
  'texlive-langcjk'
  'texlive-latex'
  'texlive-latexextra'
  'texlive-latexrecommended'
  'texlive-mathscience'
  'texlive-pictures'
  'texlive-plaingeneric'
  'texlive-xetex'
)
optdepends=(
  'texlive-fontsextra: Additional fonts'
  'drawio-desktop: Draw.io support'
  'graphviz: Graph visualization'
  'plantuml: PlantUML diagrams'
  'inkscape: SVG editing'
  'imagemagick: Image manipulation'
)
source=("${pkgname}-${pkgver}-x86_64.tar.gz::https://github.com/wang-borong/omnidoc/releases/download/v${pkgver}/${pkgname}-${pkgver}-x86_64.tar.gz")
sha256sums=('SKIP')

package() {
  cd "${srcdir}/${pkgname}-${pkgver}"
  install -Dm755 omnidoc "${pkgdir}/usr/local/bin/omnidoc"
}

