# bin/rag —— 本地结构化 RAG 工具（BM25 全文排序）

对指定目录下指定类型的文本/源码文件建立本地结构化倒排索引（默认落 `.work/rag/`），
用 BM25 对分块做全文排序检索，返回命中文件、行号范围与片段。
纯 Python 3 标准库实现：**无第三方依赖、无网络访问、无守护进程**。

## 调用契约（调用前先确认）

- **解释器**：Python 3.8+（脚本首行 shebang `#!/usr/bin/env python3`）；直接执行 `bin/rag/rag`，
  或解释器不可直接执行时用 `python3 bin/rag/rag`。
- **索引目录默认 `./.work/rag`（相对当前工作目录）**——从 swarm 根目录调用即落在 swarm 的
  `.work/rag`（宪法第一章「RAG 索引与检索」）；工作目录不确定时用 `--out` / `--index`
  显式给绝对路径（见 `packs/_common/engineering.md`「路径与工作目录确定性」）。
- **默认索引目标 `project/`**（swarm 唯一工作区）；可传位置参数指定任意目录。
- 不修改任何被索引文件；索引输出只写 `--out` 指定目录（默认 `.work/rag/`）。

## 命令速查

```sh
bin/rag/rag index project --types py,md,ts   # 对 project/ 下指定类型文件建立/增量更新索引（落 .work/rag）
bin/rag/rag index project --types '*'        # 索引全部文件（仍跳过二进制与超大文件）
bin/rag/rag index project --rebuild          # 丢弃旧索引完全重建
bin/rag/rag search "BM25 排序" --top 10      # BM25 全文排序检索（返回文件、行号、命中行片段）
bin/rag/rag search 解析 --index .work/rag --json   # 机器可读输出，供脚本消费
bin/rag/rag show 42                          # 按分块 id 打印整块原文
bin/rag/rag files                            # 列出已索引文件（词元数/分块数）
bin/rag/rag status                           # 索引是否丢失/损坏/过期（退出码 0/1/3）
bin/rag/rag verify                           # 一致性深检（结构互指 + 词频逐项对账）
bin/rag/rag help                             # 详细用法
```

常用参数：`index --out DIR`（自定义索引位置）、`--chunk-lines N` / `--overlap-lines N`
（分块粒度，默认 60 行窗口、10 行重叠）、`--include-hidden`（不跳过隐藏目录）；
`search --full`（输出整块文本而非命中行）。

## 索引结构（`.work/rag/`，均为 UTF-8 JSON / JSONL）

| 文件 | 内容 |
|---|---|
| `manifest.json` | 索引元数据：根目录、类型列表、BM25/分块参数、统计（文件/分块/词项/词元/avgdl）、`next_chunk_id`；**最后写入，存在即视为索引完整提交** |
| `files.json` | 文件账本：相对路径 → `{type,size,mtime,sha256,chunk_ids,n_tokens}`；增量更新凭 sha256 判定变更 |
| `chunks.jsonl` | 分块正文：每行 `{id,path,start_line,end_line,n_tokens,text}`（行号 1 基闭区间） |
| `postings.json` | 倒排索引：词项 → `{chunk_id: tf}`（df 现场取映射长度，不单独存储） |

索引属运行时状态（`.work/`），不进 `project/`、不进归档包；丢失可随时重建。

## 排序与分块算法

- **BM25**（k1=1.5，b=0.75，Lucene 口径 IDF）：
  `score(D,Q) = Σ_t ln(1 + (N−df+0.5)/(df+0.5)) · tf·(k1+1) / (tf + k1·(1−b+b·|D|/avgdl))`
- **分词**（索引与查询共用同一分词器）：
  拉丁/数字词整词小写保留，snake_case / camelCase 额外拆子词（子词 ≥ 2 字符）；
  CJK 连续段拆二元组（bigram），单字段落为单字词元；词元长度上限 40。
- **分块**：按行窗口切分（默认 60 行、相邻块重叠 10 行），零词元分块不入索引。
- **扫描纪律**：跳过 `.git/`、`node_modules/`、`__pycache__/`、`.work/`、`target/` 等构建
  缓存与依赖目录及隐藏目录；跳过二进制（前 8KB 含 NUL）与超过 2MB 的文件。

## 退出码

| 码 | 含义 |
|---|---|
| 0 | 成功（含 search 无命中、status 健康） |
| 1 | 错误：用法/IO/索引缺失/索引损坏（search 查询缺失索引同样为 1） |
| 3 | 索引过期（仅 `status`：文件增删或内容变更未同步进索引） |

## 与宪法的关系

制度性规则（**自动索引、检索优先走索引、中断恢复先查索引**）的唯一来源是
`CONSTITUTION.md` 第一章「RAG 索引与检索」；本文件只是工具用法手册，不承载制度条款。
