#import "@preview/i-figured:0.2.4" as i-figured
#let horizontalrule = line(start: (25%,0%), end: (75%,0%))



#show terms: it => {
  it.children
    .map(child => [
      #strong[#child.term]
      #block(inset: (left: 1.5em, top: -0.4em))[#child.description]
      ])
    .join()
}

#set table(
  inset: 6pt,
  stroke: none
)

// Three-line table implementation via show rule
#show table: it => {
  if it.has("label") and it.label == <three-line-tbl> {
     it
  } else {
     set par(justify: false)
     let f = it.fields()
     let _ = if "children" in f { f.remove("children") }
     let _ = if "stroke" in f { f.remove("stroke") }
     let pos = it.children

     let cols = f.at("columns", default: 1)
     let col-count = if type(cols) == int { cols } else if type(cols) == array { cols.len() } else { 1 }

     let header-area = 0
     let body-area = 0

     for item in pos {
        if type(item) == content and item.has("func") and item.func() == table.header {
            if item.has("children") {
                for child in item.children {
                    let c = 1
                    let r = 1
                    if child.has("func") and child.func() == table.cell {
                        let cf = child.fields()
                        c = cf.at("colspan", default: 1)
                        r = cf.at("rowspan", default: 1)
                    }
                    header-area += c * r
                }
            }
        } else if type(item) == content and item.has("func") and (item.func() == table.hline or item.func() == table.vline) {
            // Skip lines
        } else {
            let c = 1
            let r = 1
            if type(item) == content and item.has("func") and item.func() == table.cell {
                let cf = item.fields()
                c = cf.at("colspan", default: 1)
                r = cf.at("rowspan", default: 1)
            }
            body-area += c * r
        }
     }

     let header-rows = int(calc.ceil(float(header-area) / col-count))
     let total-rows = int(calc.ceil(float(header-area + body-area) / col-count))

     let new-args = ()

     new-args.push(table.hline(y: 0, stroke: 1.5pt))
     if header-rows > 0 {
         new-args.push(table.hline(y: header-rows, stroke: 0.75pt))
     }
     new-args += pos
     new-args.push(table.hline(y: total-rows, stroke: 1.5pt))

     [
       #table(..new-args, ..f, stroke: none) <three-line-tbl>
     ]
  }
}

$if(highlighting-definitions)$
// syntax highlighting functions from skylighting:
$highlighting-definitions$

$endif$

// Embedded conf function with ctexart customizations
#let content-to-string(content) = {
  if content.has("text") {
    content.text
  } else if content.has("children") {
    content.children.map(content-to-string).join("")
  } else if content.has("body") {
    content-to-string(content.body)
  } else if content == [ ] {
    " "
  }
}

// GitHub-style Admonitions
#let admonition-dict = (
  "NOTE": (color: blue, title: "Note"),
  "TIP": (color: green, title: "Tip"),
  "IMPORTANT": (color: purple, title: "Important"),
  "WARNING": (color: orange, title: "Warning"),
  "CAUTION": (color: red, title: "Caution"),
)

// --- Helper Functions ---

#let make-toc(
  toc: false,
  toc-title: none,
  toc-depth: none,
  toc-own-page: true,
  toc-leading: 1em,
  is-zh: false,
  font: (),
  weight: "bold"
) = {
  if toc {
    let t-title = if toc-title != none { toc-title }
        else if is-zh { "目 录" }
        else { "Table of Contents" }

    let content = {
      align(center)[#heading(numbering: none, outlined: false)[#t-title]]

      show outline.entry.where(level: 1): it => {
        v(1.5em, weak: true)
        set text(font: font, weight: weight)
        it
      }

      set par(leading: toc-leading)
      outline(title: none, depth: toc-depth)
    }

    if toc-own-page {
      page(header: none, footer: none, content)
    } else {
      content
      v(5em)
    }
  }
}

#let make-cover(
  cover: false,
  title: none,
  subtitle: none,
  authors: (),
  date: none,
  abstract: none,
  abstract-title: none,
  font: (), // Main or Heading font
  weight: "bold",
  title-size: 36pt,
  subtitle-size: 26pt,
  is-zh: false
) = {
  if cover {
     page(header: none, footer: none, align(center + horizon)[
        #if title != none {
            text(weight: weight, size: title-size, font: font)[#title]
        }
        #if subtitle != none {
            parbreak()
            text(weight: weight, size: subtitle-size)[#subtitle]
        }
        #if authors != none and authors != [] {
          v(2em)
          let count = authors.len()
          let ncols = calc.min(count, 3)
          grid(
            columns: (1fr,) * ncols,
            row-gutter: 1.5em,
            ..authors.map(author => align(center)[
              #author.name \
              #author.affiliation \
              #author.email
            ])
          )
        }
        #if date != none {
          v(2em)
          date
        }
     ])
     pagebreak()
  }
}

#let conf(
  title: none,
  subtitle: none,
  authors: (),
  keywords: (),
  date: none,
  abstract-title: none,
  abstract: none,
  thanks: none,
  cols: 1,
  margin: (x: 1.5in, y: 1.5in),
  paper: "a4",
  lang: "zh",
  region: "CN",
  font: "auto",
  fontsize: 10.5pt, // 5号字 (10.5pt) is standard for ctexart
  mathfont: "auto",
  codefont: ("FiraCode Nerd Font", "SimHei", "Source Han Sans"),
  linestretch: 1.25,
  sectionnumbering: "1.1",
  linkcolor: none,
  citecolor: none,
  filecolor: none,
  toccolor: none,
  pagenumbering: "1",
  toc-own-page: false,
  toc: false,
  toc-title: none,
  toc-depth: none,
  toc-leading: 1em,
  header-text: "auto",
  heading-spacing-below: 1.5em,
  heading-spacing-l1: 3em,
  heading-spacing-l2: 2em,
  heading-spacing-l3: 1.5em,
  heading-spacing-l4: 1.25em,
  heading-spacing-l5: 1.25em,
  list-indent: 1em,
  figure-caption-position: "bottom",
  table-caption-position: "top",
  horizontalrule-spacing: 3em,
  cover: false,
  doc,
) = {
  // Bilingual setup
  let is-zh = lang.starts-with("zh")

  // Font defaults based on language
  let main-font = if font != "auto" {
    font
  } else if is-zh {
    ("Times New Roman", "SimSun", "Noto Serif CJK SC", "Noto Serif")
  } else {
    ("Times New Roman", "Noto Serif")
  }

  let heading-font = if is-zh {
    ("Times New Roman", "SimHei", "Noto Sans CJK SC")
  } else {
    ("Times New Roman",)
  }

  let heading-weight = if is-zh { "regular" } else { "bold" }

  let title-weight = heading-weight // Unified title weight
  let title-size = 26pt // Unified (Cover)
  let subtitle-size = 22pt // Unified (Cover)
  let body-title-size = 22pt // Unified (Body)
  let body-subtitle-size = 18pt // Unified (Body)

  // Localized terms
  let abstract-default-title = if is-zh { "摘 要" } else { "Abstract" }
  let code-supplement = if is-zh { "代码" } else { "Code" }

  set document(
    title: title,
    keywords: keywords,
  )
  set document(
      author: authors.map(author => content-to-string(author.name)).join(", ", last: " & "),
  ) if authors != none and authors != ()

  set page(
    paper: paper,
    margin: margin,
    numbering: pagenumbering,
    header: context {
        if here().page() == 1 {
            none
        } else {
             show emph: it => {
                  set text(font: ("Times New Roman", "KaiTi", "AR PL New Kai",), style: "normal")
                  show regex("[\x00-\x7F]+"): set text(font: "Times New Roman", style: "italic")
                  it.body
             }

             if header-text == "auto" {
                let here_loc = here()
                let all_headings = query(heading.where(level: 1))
                let page_headings = all_headings.filter(h => h.location().page() == here_loc.page())

                let target = if page_headings.len() > 0 {
                     page_headings.first()
                } else {
                     let prev = all_headings.filter(h => h.location().page() < here_loc.page())
                     if prev.len() > 0 { prev.last() } else { none }
                }

                if target != none {
                   let el = target
                   let body = if el.numbering != none {
                        numbering(el.numbering, ..counter(heading).at(el.location())) + h(0.75em) + el.body
                   } else {
                        el.body
                   }
                   align(left, emph(body))
                }
             } else {
                if header-text != none and header-text != [] and header-text != "" {
                    align(left, emph(header-text))
                }
             }
        }
    },
    // columns: cols -- Removed to allow per-section column control
  )

  // Spacing Configuration
  // let heading-above = 2em
  // let heading-below = 1em
  let block-spacing = 1.5em

  // Paragraph settings
  set par(
    justify: true,
    first-line-indent: (amount: 2em, all: true),
    leading: linestretch * 1em
  )

  // Font settings
  set text(lang: lang,
           font: main-font,
           region: region,
           size: fontsize)


  // Math Numbering
  set math.equation(numbering: "(1)")
  show math.equation: set text(font: mathfont) if mathfont != "auto"
  show raw: set text(font: codefont) if codefont != none

  // Code block frame styling
  show raw.where(block: true): block.with(
    above: block-spacing,
    below: block-spacing,
    fill: luma(240),
    inset: 10pt,
    radius: 4pt,
    stroke: (paint: luma(180), thickness: 1pt),
    width: 100%,
  )

  // Admonitions (Blockquote styling)
  show quote.where(block: true): it => {
    let content_str = content-to-string(it.body)
     let match = content_str.trim().match(regex("^\[!([A-Z]+)\]"))
     if match != none {
         let type = match.captures.first()
         let config = admonition-dict.at(type, default: none)
         if config != none {
             let (color, title) = config
             block(
               fill: color.lighten(90%),
               stroke: (left: 4pt + color),
               inset: 1em,
               width: 100%,
               radius: 2pt,
             )[
               #text(weight: "regular", fill: color)[#title]
               // Regex replace to remove the tag
               #show regex("^\[![A-Z]+\]"): none
               #it.body
             ]
             return
         }
     }
     pad(left: 1em, it)
  }

  // Extended TODO markers
  show regex("\[([>:/\?!])\]"): it => {
    let text-content = content-to-string(it)
    let char = text-content.at(1)

    let content = if char == ">" { "🏁" }
      else if char == ":" { "⏸️" }
      else if char == "/" { "🛑" }
      else if char == "?" { "❓" }
      else if char == "!" { "🔥" }
      else { none }

    if content != none {
       text(content)
    } else {
       it
    }
  }

  // Standard Checkbox Replacements
  show regex("[\u{2612}]"): "✅"
  show regex("[\u{2610}]"): "⬜"

  // i-figured setup (Must be before specific show figure styling)
  show heading.where(level: 1): i-figured.reset-counters
  show figure: i-figured.show-figure.with(numbering: "1-1")

  // Horizontal Rule spacing
  show line.where(start: (25%,0%), end: (75%,0%)): it => block(above: horizontalrule-spacing, below: horizontalrule-spacing, it)

  // Caption styling
  let table-pos = if table-caption-position == "bottom" { bottom } else { top }
  let fig-pos = if figure-caption-position == "top" { top } else { bottom }

  show figure.where(kind: "i-figured-table"): set figure.caption(position: table-pos)
  show figure.where(kind: "i-figured-image"): set figure.caption(position: fig-pos)
  show figure.where(kind: "i-figured-raw"): set figure.caption(position: top)
  show figure: set block(above: block-spacing, below: block-spacing)
  show figure.caption: set text(font: main-font, size: 9pt)
  // Fix indentation for captions (reset to 0, force all: false)
  show figure.caption: set par(first-line-indent: (amount: 0pt, all: false))
  // Center all captions
  show figure.caption: set align(center)

  // Enable code supplement localization
  show figure.where(kind: "i-figured-raw"): set figure(supplement: code-supplement)

  // TOC settings
  show outline: set text(font: main-font)
  show outline: set text(fill: rgb(content-to-string(toccolor))) if toccolor != none

  // Math Numbering
  set math.equation(numbering: "(1)")

  // Heading Styles
  show heading: set text(font: heading-font, weight: heading-weight)
  show heading.where(level: 1): set align(center)
  show heading: set block(below: heading-spacing-below)
  show heading.where(level: 1): set block(above: heading-spacing-l1)
  show heading.where(level: 2): set block(above: heading-spacing-l2)
  show heading.where(level: 3): set block(above: heading-spacing-l3)
  show heading.where(level: 4): set block(above: heading-spacing-l4)
  show heading.where(level: 5): set block(above: heading-spacing-l5)
  show heading: it => it // explicit passthrough not needed with set block, but to ensure no other wrapper breaks
  set heading(numbering: (..nums) => numbering(sectionnumbering, ..nums) + h(0.75em))

  // List Indentation
  set list(indent: list-indent)
  set enum(indent: list-indent)

  show link: set text(fill: rgb(content-to-string(linkcolor))) if linkcolor != none
  show ref: set text(fill: rgb(content-to-string(citecolor))) if citecolor != none
  show link: this => {
    if filecolor != none and type(this.dest) == label {
      text(this, fill: rgb(content-to-string(filecolor)))
    } else {
      text(this)
    }
  }

  // Title Logic
  make-cover(
    cover: cover,
    title: title,
    subtitle: subtitle,
    authors: authors,
    date: date,
    font: main-font,
    weight: title-weight,
    title-size: title-size,
    subtitle-size: subtitle-size,
    is-zh: is-zh
  )

  if cover {
    counter(page).update(1)
  }

  if cover and abstract != none {
    let abstract_title_text = if abstract-title != none { abstract-title } else if is-zh { "摘 要" } else { "Abstract" }
    align(center)[#heading(level: 1, numbering: none, outlined: false)[#abstract_title_text]]
    block(inset: (left: 2em, right: 2em))[
      #abstract
    ]
    pagebreak()
  }

  if not cover {
      block(below: 4em, width: 100%)[
        #if title != none {
          align(center, block[
              #text(weight: title-weight, size: body-title-size, font: main-font)[#title #if thanks != none {
                  footnote(thanks, numbering: "*")
                  counter(footnote).update(n => n - 1)
                }]
              #(
                if subtitle != none {
                  parbreak()
                  text(weight: title-weight, size: body-subtitle-size)[#subtitle]
                }
               )])
        }

        #if authors != none and authors != [] {
          let count = authors.len()
          let ncols = calc.min(count, 3)
          grid(
            columns: (1fr,) * ncols,
            row-gutter: 1.5em,
            ..authors.map(author => align(center)[
              #author.name \
              #author.affiliation \
              #author.email
            ])
          )
        }

        #if date != none {
          align(center)[#block(inset: 1em)[
              #date
            ]]
        }

        #if abstract != none {
          let abstract_title_text = if abstract-title != none { abstract-title } else { abstract-default-title }
          block(inset: 2em)[
            #text(weight: heading-weight, font: heading-font)[#abstract_title_text] #h(1em) #abstract
          ]
        }
      ]
  }

  make-toc(
    toc: toc,
    toc-title: toc-title,
    toc-depth: toc-depth,
    toc-leading: toc-leading,
    toc-own-page: toc-own-page,
    is-zh: is-zh,
    font: heading-font,
    weight: heading-weight
  )

  if toc {
    counter(page).update(1)
  }

  if is-zh and font == "auto" {
    show strong: it => {
       set text(font: ("Times New Roman", "SimHei"), weight: "regular")
       show regex("[\x00-\x7F]+"): set text(weight: "bold")
       it.body
    }
    show emph: it => {
      set text(font: ("Times New Roman", "KaiTi"), style: "normal")
      show regex("[\x00-\x7F]+"): set text(style: "italic")
      it.body
    }
    if cols > 1 { columns(cols, doc) } else { doc }
  } else {
    if cols > 1 { columns(cols, doc) } else { doc }
  }
}

$if(smart)$
$else$
#set smartquote(enabled: false)

$endif$
$for(header-includes)$
$header-includes$

$endfor$
#show: doc => conf(
$if(title)$
  title: [$title$],
$endif$
$if(subtitle)$
  subtitle: [$subtitle$],
$endif$
$if(author)$
  authors: (
$for(author)$
$if(author.name)$
    ( name: [$author.name$],
      affiliation: [$author.affiliation$],
      email: [$author.email$] ),
$else$
    ( name: [$author$],
      affiliation: "",
      email: "" ),
$endif$
$endfor$
    ),
$endif$
$if(keywords)$
  keywords: ($for(keywords)$"$it$"$sep$,$endfor$),
$endif$
$if(date)$
  date: [$date$],
$endif$
$if(lang)$
  lang: "$lang$",
$endif$
$if(region)$
  region: "$region$",
$endif$
$if(abstract-title)$
  abstract-title: [$abstract-title$],
$endif$
$if(header-text)$
  header-text: [$header-text$],
$endif$
$if(abstract)$
  abstract: [$abstract$],
$endif$
$if(thanks)$
  thanks: [$thanks$],
$endif$
$if(margin)$
  margin: ($for(margin/pairs)$$margin.key$: $margin.value$,$endfor$),
$endif$
$if(papersize)$
  paper: "$papersize$",
$endif$
$if(mainfont)$
  font: ("$mainfont$",),
$endif$
$if(fontsize)$
  fontsize: $fontsize$,
$endif$
$if(mathfont)$
  mathfont: ($for(mathfont)$"$mathfont$",$endfor$),
$endif$
$if(codefont)$
  codefont: ($for(codefont)$"$codefont$",$endfor$),
$endif$
$if(linestretch)$
  linestretch: $linestretch$,
$endif$
$if(section-numbering)$
  sectionnumbering: "$section-numbering$",
$endif$
  pagenumbering: $if(page-numbering)$"$page-numbering$"$else$none$endif$,
$if(linkcolor)$
  linkcolor: [$linkcolor$],
$endif$
$if(citecolor)$
  citecolor: [$citecolor$],
$endif$
$if(filecolor)$
  filecolor: [$filecolor$],
$endif$
$if(toccolor)$
  toccolor: [$toccolor$],
$endif$
$if(toc-own-page)$
  toc-own-page: $toc-own-page$,
$endif$
$if(cover)$
  cover: $cover$,
$endif$
  cols: $if(columns)$$columns$$else$1$endif$,
  toc: $if(toc)$true$else$false$endif$,
  toc-title: $if(toc-title)$[$toc-title$]$else$none$endif$,
  toc-depth: $if(toc-depth)$$toc-depth$$else$none$endif$,
$if(heading-spacing-below)$
  heading-spacing-below: $heading-spacing-below$,
$endif$
$if(heading-spacing-l1)$
  heading-spacing-l1: $heading-spacing-l1$,
$endif$
$if(heading-spacing-l2)$
  heading-spacing-l2: $heading-spacing-l2$,
$endif$
$if(heading-spacing-l3)$
  heading-spacing-l3: $heading-spacing-l3$,
$endif$
$if(heading-spacing-l4)$
  heading-spacing-l4: $heading-spacing-l4$,
$endif$
$if(heading-spacing-l5)$
  heading-spacing-l5: $heading-spacing-l5$,
$endif$
$if(list-indent)$
  list-indent: $list-indent$,
$endif$
$if(toc-leading)$
  toc-leading: $toc-leading$,
$endif$
$if(figure-caption-position)$
  figure-caption-position: "$figure-caption-position$",
$endif$
$if(table-caption-position)$
  table-caption-position: "$table-caption-position$",
$endif$
$if(horizontalrule-spacing)$
  horizontalrule-spacing: $horizontalrule-spacing$,
$endif$
  doc,
)

$for(include-before)$
$include-before$

$endfor$

$body$

$if(citations)$
$for(nocite-ids)$
#cite(label("${it}"), form: none)
$endfor$
$if(csl)$

#set bibliography(style: "$csl$")
$elseif(bibliographystyle)$

#set bibliography(style: "$bibliographystyle$")
$endif$
$if(bibliography)$

#bibliography(($for(bibliography)$"$bibliography$"$sep$,$endfor$)$if(full-bibliography)$, full: true$endif$)
$endif$
$endif$
$for(include-after)$

$include-after$
$endfor$
