# PMD（CPD/DRY）专项注意事项

> 用途分类：PMD 发行版工具（当前口径 7.28.0）的机制细节与陷阱。调用选型、
> 工具安装位置与「零发现先做阳性对照」等通用条款见 `packs/_common/engineering.md`。

- **`pmd cpd` 的退出码 4 = 发现重复，不是处理错误**（7.28.0 实测）：PMD 7 的 CPD
  以退出码区分「无重复」（0）与「发现重复」（4）——按老印象把 4 当异常重跑或忽略
  结果会误判；先看报告内容再归因退出码。
- **XML 报告带命名空间，朴素 ElementTree 解析得 0 条**：根元素
  `<pmd-cpd xmlns="https://pmd-code.org/schema/cpd-report">`——不带 namespace 参数
  的 `findall('duplication')` 静默匹配空集，表象是「text 格式有发现、XML 解析 0 条」
  的矛盾。解析传 `namespaces={'c': 'https://pmd-code.org/schema/cpd-report'}` 或改用
  text/json 渲染器。
- **Rust 必须显式 `-l rust`**：不带语言参数时默认只收 java，静默报 0（通用条款的
  阳性对照现场：先用一处已知重复验证过滤参数生效，再采信零发现）。
