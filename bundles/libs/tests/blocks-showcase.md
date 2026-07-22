---
title: OmniDoc Semantic Blocks
lang: zh-CN
documentclass: ctexart
geometry: margin=20mm
---

# 语义容器

::: {.admonition .note}
这是用于背景和补充信息的说明框，支持普通文本、`行内代码` 和列表。

- 第一项
- 第二项
:::

::: {.admonition .tip title="设计提示"}
优先画出小信号等效模型，再判断哪些电容在目标频段可以视为短路。
:::

::: {.admonition .important}
闭环稳定性必须在最坏负载和工艺角下验证。
:::

::: {.admonition .warning}
上电前检查电源极性和限流设置。
:::

::: {.admonition .error}
不要在没有共地的情况下直接连接两台台式仪器的信号端。
:::

::: {.admonition .question}
为什么负反馈通常能够降低增益对器件参数的敏感度？
:::

::: {.admonition .answer}
因为闭环增益主要由反馈网络决定；当环路增益足够大时，前向增益变化被反馈抑制。
:::

::: {.admonition .example}
一个 10 倍反相放大器可取 $R_{in}=10\,\mathrm{k}\Omega$、$R_f=100\,\mathrm{k}\Omega$。
:::

::: {.admonition .exercise}
计算上述放大器在输入为 $0.2\,\mathrm{V}$ 时的理想输出。
:::

::: {.admonition .solution}
理想输出为 $-2\,\mathrm{V}$。
:::
