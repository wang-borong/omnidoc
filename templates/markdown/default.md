---
title: {{ title }}
author:
  - {{ author }}
date:
  - {{ date }}

{{ doctype }}

indent: true
listings: true
numbersections:
  - sectiondepth: 5

#biblatex: true
#biblatexoptions:
#  - backend=biber
#  - citestyle=numeric-comp
#  - bibstyle=numeric
#bibliography:
#  - biblio/cseebook.bib
#nocite-ids:
#  - *
#biblio-title: 参考文献
#csl: computer.csl

csl: computer.csl
#colorlinks: true
graphics: true

toc: true
lof: true
lot: true

header-includes:
  - |
    ```{=latex}
    {{ latex_header }}
    ```

include-before:
  - |
    ```{=latex}
    ```
include-after:
  - |
    ```{=latex}
    ```
before-body:
  - |
    ```{=latex}
    ```
after-body:
  - |
    ```{=latex}
    ```
...

```{.include}

```


