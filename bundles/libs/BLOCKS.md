# OmniDoc fenced block 语法

OmniDoc 对 Pandoc Markdown 提供三组正式扩展：语义容器、源码包含和可渲染图形。这里列出的写法构成公共语法；没有列出的历史 filter 不属于公共接口。

## 语义容器

统一写法为 fenced Div：

```markdown
::: {.admonition .warning title="上电前检查"}

确认电源极性和限流设置正确，再给电路上电。

:::
```

类型必须从下表选择。`title` 可省略；省略时会根据文档 `lang` 自动使用中文或英文标题。

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

## 输出格式

图形渲染器按目标格式自动选择资源：

| 目标 | 图形格式 |
|---|---|
| PDF、LaTeX | PDF |
| HTML、EPUB | SVG |
| DOCX、PPTX | PNG |

语义容器在 DOCX、PPTX 中保留结构和正文，但精细的主题视觉主要面向 PDF、HTML 和 EPUB。
