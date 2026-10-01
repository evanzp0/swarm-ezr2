# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（流程末位）　会话：qa-20261001（phase-01 最终验证与 QA 可执行化）

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器，正式版一期。
- 需求权威来源：`project/mission.md`（v1.1 大纲）+ `project/mission/phase-01.md`（v1.2 详述）。

### 产物总账

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | `features/` 9 份 Gherkin（114 场景）+ `qa/` 9 份规程 | 已交付 | 三层一致 |
| coder | `project/ezr/` 下载内核+TUI+CLI+fixture | 已交付（P0+P1 全量） | 100 单测、clippy 0 警告 |
| cleaner/architect/hardender | 清理/架构/加固 | 已交付 | 归档在案 |
| QA（本会话） | `project/qa/runners/` 9 套可执行套件（114 用例）+ 产品缺陷修复 10 处 + QA 基建 ezr-proxy 修复 2 处 | 已交付 | 最终 110 P + 4 S / 0 F（详见下） |

### 实现定义值登记

- 沿用前轮全部登记（toast 文案、证书错误文案、v0.1.0-01）。
- 新增：多伴随校验择序 = 算法表声明序（MD5 在前）；default_concurrency 非法值钳制 1–64。

## 二、当前产出情况

### 本会话产物清单

- `project/qa/runners/`：harness.py（共享基建：EzrApp 伪终端驱动/Fixture/Suite/共享操作助手）
  + 9 套件（download-engine 16、add-task 17、integrity-check 11、resume-sidecar 13、
  persistence-config 9、slots-queue 13、tui-display 14、throttle-proxy 11、retry-backoff 10，
  合计 114 用例）。
- 产品缺陷修复 10 处（全部最小改动、clippy 0 警告、100 单测回归通过）：
  1. supervisor.rs：流式循环回填 ConnView.done（FR-01-81 每连接进度/速度，QA-DE-06）
  2. app.rs：对话框并发非 1–64 一律回退默认（Gherkin 01-add-task-06）
  3. app.rs：重复任务守卫（同 URL 同目录 toast「任务已存在」，01-add-task-16）
  4. app.rs：Evt::Invalidated 释放 has_slot（FR-01-22 失效后自动重下断链）
  5. app.rs：requeue_failed 重置 invalidation_streak（01-resume-sidecar-09）
  6. app.rs：Probed CD 名回写重新去重（01-add-task-10 原文件覆盖级数据丢失）
  7. engine/supervisor.rs：CD 定稿名盘上去重（同上，引擎写路径侧）
  8. engine/supervisor.rs：下载前 create_dir_all 保存目录（01-add-task-11 目录自动创建）
  9. main.rs：CLI -c 范围校验（01-add-task-14）+ 多 URL 部分非法全部拒绝（01-add-task-15）
  10. app.rs：倒计时到点补发 Start（探针失败型任务自动重试断链，FR-01-41，QA-RB-03/04）
      + ui.rs：auto_retry=false 显示「不自动重试」而非「已达上限」（01-retry-backoff-10）

### 验证证据（9 套件分块/全量执行，P/F/S）

| 套件 | 结果 | 备注 |
|---|---|---|
| 01-download-engine | 16 P | DE-03 补门控防瞬时完成竞速 |
| 01-add-task | 17 P | 含 bracketed paste、CLI 参数族 |
| 01-integrity-check | 11 P | 7 算法 + 伴随文件族 + 校验流转 |
| 01-resume-sidecar | 13 P | kill -9 ×3、一致性失效链、删除语义 |
| 01-persistence-config | 9 P | 单实例锁、非法配置回退、目录解析 |
| 01-slots-queue | 13 P | 槽位调度、排队、六态 |
| 01-tui-display | 14 P | 四区结构、状态色、鼠标 SGR、CJK |
| 01-retry-backoff | 9 P + 1 S | RB-04 退避序列 8→16→32→60→60 实测 222s |
| 01-throttle-proxy | 8 P + 3 S | TP-01 100MB 总量口径验证 AC-7 ±10%；TP-08/10/11 代理路径实跑 P（见下）；剩余 3 S 均测量方法学 |
- 基线：cargo test 100 P / 0 F；clippy --all-targets 0 警告。
- CRAP：46/200 函数超阈值 30——主体为 UI 交互层（本会话 9 套件 114 用例端到端覆盖，
  不在 cargo-crap 覆盖视野内）；逻辑层函数均有单测。无进一步行动项。
- DRY：PMD CPD 70-token 产品代码 0 重复块。

### 待办与待批（原 1–4 已裁决落地，本节仅余基建注记）

1. ~~QA-IC-06~~ **已裁决（改规格）**：Gherkin 场景 06/规程/套件注记均已改为
   「按算法表声明顺序取最先（MD5 在表首）」，与实现 CHECKSUM_ALGOS 一致。
2. ~~QA-PC-04~~ **已裁决（改规格）**：Gherkin 场景 04 标题改为「回退默认或钳制边界」、
   default_concurrency=99 期望 64；规程同步。套件改用 sidecar.concurrency 断言
   （详情并发为活跃连接数，随限速/块完成波动，不作配置值断言口径）。
3. ~~QA-01-03~~ **已裁决（改规程文案）**：Gherkin 场景 03/规程改为逐键过滤语义
   （纯非法输入字段留空；确认无格式报错；空值按无校验创建）。
4. ~~QA-IC-11~~ **已裁决（改规程文案）**：Gherkin 场景 11/规程改为「网络类瞬态 +
   续传补齐主链」；「文件大小不符」文案标注为块记账不变式下不可达。
5. **基建注记（phase-02 建议项）**：ezr-fixture 对每个请求把整文件读入内存
   （64 并发 × big-100m.bin ≈ 6.4GB 触发 OOM-kill，dmesg 实证）。QA 用例设计
   应避开「大文件 × 高并发」组合（PC-04 已按 ten-m+128KB 块规避）；fixture
   改为按 Range 流式读盘可根除，列入 phase-02 基建优化。
5. ~~疑似缺陷（TP-08/10/11 记 S）：代理路径挂起~~ **已裁决并修复（本会话复查）**：
   根因在 QA 基建 ezr-proxy 两处分发缺陷（产品引擎代理链路经原始 TCP 对照实验排除）：
   ① accept 分发线程已消费请求头，handle_http 仍在流上重读 → 永久阻塞、不转发不记日志
   （任务停在「等待中」的直接原因）；② CONNECT 目标误传整条请求行（hostport 解析为
   "CONNECT" → 502）。修复后 TP-08（config 代理优先于环境变量）/TP-10（CONNECT 隧道
   HTTPS 下载）/TP-11（带凭据代理无泄露）三用例转实跑全 P；TP-05 亦由 S 转实跑 P
   （套件口径修复：限速档独立子目录隔离脏测与重复守卫）。最终总账：114 用例 = 110 P + 4 S / 0 F。
6. **TP-02/03/06 记 S（测量方法学）**：loopback 短传输绝对速率受启动瞬态支配；
   限速器生效语义已由 TP-01（100MB 总量口径）权威验证；十进制解析由 parse_speed
   单测覆盖；各档位总耗时口径已由 TP-05 实跑覆盖。

### 移交建议

- six-pack 流转路径 specifier → coder → cleaner → architect → hardender → QA 已走完，
  phase-01 工作流完结。
- 建议收尾：`./bin/swarm complete`（构建缓存 target/ 由 complete 自动清理）。
- phase-02 开工时：原待办 1–5 已全部裁决落地（1–4 见上，5 代理路径已修复实跑）；
  剩余基建项：ezr-fixture Range 流式化（见待办 5 注记），非阻塞。
