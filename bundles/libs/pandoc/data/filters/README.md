# OmniDoc Pandoc filters

公共 fenced block 语法、属性和示例统一记录在项目根目录的 [`BLOCKS.md`](../../../BLOCKS.md)。新增公共语法时必须同时满足：

1. PDF、HTML 和 EPUB 的行为已定义；
2. 有自动化构建测试；
3. 图源和外部输入进入依赖图；
4. `BLOCKS.md` 已更新。

过滤器源文件中的注释用于维护实现，不再作为分散的用户文档。

当前默认链路包括：

- `include-files.lua`：章节包含；
- `include-code-files.lua`：源码包含；
- `diagram-generator.lua`：图形块；
- `admonition.lua`：语义容器；
- `display-math.lua`：HTML/EPUB 独立公式布局；
- `latex-headers.lua`、`latex-patch.lua`：LaTeX writer 集成；
- `emoji.lua`、`fonts-and-alignment.lua`：跨格式文本支持。
