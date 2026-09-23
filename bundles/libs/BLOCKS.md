# OmniDoc fenced block 语法

OmniDoc 对 Pandoc Markdown 提供三组正式扩展：语义容器、源码包含和可渲染图形。这里列出的写法构成公共语法；没有列出的历史 filter 不属于公共接口。

## 语义容器

统一写法为 fenced Div：

```markdown
::: {.admonition .warning title="上电前检查"}

确认电源极性和限流设置正确，再给电路上电。

:::
```

类型必须从下表选择。`title` 可省略；省略时会根据文档 `lang` 自动使用中文或英文标题。标题按单行行内 Markdown 解析，因此支持强调、行内代码和 `$...$` 行内公式：

```markdown
::: {.admonition .tip title="工程速算：把 $g_m$ 翻成一把 $1/g_m$ 电阻尺"}

把跨导换算为阻抗量级，可以快速判断端口上最强的局部钳位。

:::
```

| 类型 | 用途 | 中文默认标题 |
|---|---|---|
| `note` | 补充说明、背景信息 | 说明 |
| `tip` | 技巧、提示、捷径 | 提示 |
| `important` | 必须注意的关键结论 | 重要 |
| `warning` | 可能导致错误结果或设备风险 | 警告 |
| `error` | 已知错误、禁止操作、失败原因 | 错误 |
| `question` | 问题、思考题的题干 | 问题 |
| `answer` | 简短回答或结论 | 回答 |
| `example` | 示例和例题说明 | 示例 |
| `exercise` | 练习或待完成任务 | 练习 |
| `solution` | 练习的完整解答 | 解答 |

问答建议成对书写：

```markdown
::: {.admonition .question}

为什么理想运放在线性区满足虚短？

:::

::: {.admonition .answer}

负反馈使差模输入电压被压低；虚短是闭环高增益条件下的近似，而不是器件端口真的短接。

:::
```

PDF 使用统一的 `omni-blocks` LaTeX 模块；HTML 和 EPUB 使用 `semantic-blocks.css`。颜色、间距、标题和标记在三种输出中保持一致语义。

## 源码和章节包含

包含章节：

````markdown
```{.include shift-heading-level-by=1}
chapters/introduction.md
```
````

包含源码：

````markdown
```{.python include-code="scripts/analyse.py" start-line=20 end-line=48 dedent=4 numberLines}
```
````

常用属性：

- `include-code`：源码路径；
- `start-line`、`end-line`：包含范围；
- `dedent`：统一移除的前导空格数；
- `numberLines`：显示行号；
- `shift-heading-level-by`：章节包含后的标题级别偏移。

OmniDoc 会把成功读取的章节和源码写入依赖图，因此它们参与缓存和 lock 摘要计算。

## 可渲染图形

所有图形块共享以下属性：

- `#fig-id`：稳定且唯一的图号标识；
- `caption`：图题；
- `width`、`height`：Pandoc 图像尺寸；
- `include-code`：将较长图源保存在独立文件中，并纳入依赖跟踪。

### 电路图

````markdown
```{.circuit #fig-divider include-code="schematics/divider.py"
caption="电阻分压电路" width="70%"}
```
````

图源使用 Schemdraw，并预置 `d`（Drawing）和 `elm`（elements）。图源是可信 Python 代码，只应构建受信任的文档仓库。

### SPICE 曲线

````markdown
```{.spiceplot #fig-response include-code="sim/response.json"
caption="输出电压扫描结果" width="82%"}
```
````

JSON 必须包含：

```json
{
  "netlist": "sim/example.cir",
  "analysis": "tran 10u 20m",
  "traces": [{"expr": "v(out)", "label": "输出"}]
}
```

可选字段包括 `xlabel`、`ylabel`、`title`、`xscale`、`yscale`、`x_multiplier`、`figsize` 和 `legend`。网表也会进入依赖图。

### Matplotlib 函数曲线

`matplot` 块执行受信任的 Python 绘图片段。渲染器预置 NumPy、Pyplot、画布和坐标轴，分别命名为 `np`、`plt`、`fig` 和 `ax`；代码只需描述数据与图形，图片会按目标格式自动保存：

~~~~markdown
~~~{.matplot #fig-tanh caption="双曲正切函数" width="82%"}
x = np.linspace(-4.0, 4.0, 801)
ax.plot(x, np.tanh(x), linewidth=2.2, label=r"$\tanh(x)$")
ax.axhline(1.0, color="0.45", linestyle=":")
ax.axhline(-1.0, color="0.45", linestyle=":")
ax.set(xlabel=r"$x$", ylabel=r"$y$", xlim=(-4, 4), ylim=(-1.1, 1.1))
ax.grid(True, alpha=0.25)
ax.legend(frameon=False)
~~~
~~~~

不要在片段中调用 `savefig`；OmniDoc 会保存最终的 `fig`。如需自定义布局，可以重新赋值 `fig`/`ax`，也可以通过 `include-code` 引入独立的 `.py` 文件。构建环境需提供 NumPy 与 Matplotlib；Python 解释器由项目的 `[tools].python3` 或 `PATH` 决定。

和 `circuit`、`py2image` 一样，`matplot` 会执行文档仓库中的代码，因此只应构建受信任的内容。

### 寄存器位域

````markdown
```{.bitfield #fig-control caption="控制寄存器" width="100%"}
{
  "bits": 8,
  "entries": [
    {"name": "VALUE", "bits": 7},
    {"name": "READY", "bits": 1}
  ]
}
```
````

### PlantUML、Graphviz、TikZ、Asymptote 和 Python 图形

对应类名为：

- `plantuml`
- `graphviz`
- `tikz`
- `asymptote`
- `py2image`

示例：

````markdown
```{.graphviz #fig-flow caption="信号处理流程" width="75%"}
digraph G { input -> amplifier -> output }
```
````

`py2image` 同样执行可信 Python 代码。外部渲染器必须能通过项目 `[tools]` 配置或 `PATH` 找到。

PDF/LaTeX 输出默认采用 `bounded` 浮动策略：生成图按源码位置锚定，不会越过其后的正文或表格；一级、二级标题也会清空上一节尚未落位的普通浮动体。未显式给出 `height` 的生成图最高为正文高度的 72%，避免图片连同标题超过可排版页高。项目可在 YAML 元数据中调整：

```yaml
omnidoc-float-policy: bounded       # bounded | strict | section | diagram | "off"
omnidoc-float-barrier-level: 2      # 0 表示关闭标题屏障
omnidoc-diagram-max-height: 72%     # none 表示不设置默认限高
```

`strict` 还会在每个普通 Pandoc Figure 后放置屏障；`section` 只保留标题屏障；`diagram` 只锚定生成图。单个生成图可显式设置 `height` 覆盖默认限高。PDF/LaTeX 输出将 `width` 与 `height` 作为最大尺寸，按原始比例缩放图片，避免指定宽度时被默认限高拉伸。原生 LaTeX 工程可加载 `\usepackage[subsection]{omni-floats}` 获得相同的全局参数和标题边界约束。

## 输出格式

图形渲染器按目标格式自动选择资源：

| 目标 | 图形格式 |
|---|---|
| PDF、LaTeX | PDF |
| HTML、EPUB | SVG |
| DOCX、PPTX | PNG |

语义容器在 DOCX、PPTX 中保留结构和正文，但精细的主题视觉主要面向 PDF、HTML 和 EPUB。
