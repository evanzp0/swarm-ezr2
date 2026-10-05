#!/usr/bin/env bash
# arch_check.sh — EZR 自动化架构边界检查（six-pack/architect 交付）
#
# 依据 packs/_common/notes/rust.md「架构边界与适配器方向」条款的 grep 级方案：
# 零依赖、可读、易维护，覆盖本项目十条分层规则；CI 可作为 && 链一环集成。
# （v1.3 后复核批次，architect 第二轮：新增规则 7/8，见 POSITIVE-CONTROL 尾注）
#
# 分层基线（依赖方向：低层指向高层，model 为最内层纯逻辑）：
#   main.rs → app → engine → model；ui → app + model；model → ∅
#
# 用法：bash scripts/arch_check.sh   （在 ezr crate 根目录执行；任何规则命中
#       违规即非零退出。POSITIVE-CONTROL 验证见文件尾注释）

set -u
cd "$(dirname "$0")/.." || exit 1   # 固定到 crate 根（路径确定性，engineering.md）

fails=0

# 规则 1【依赖方向】model 为最内层：不得引用 app/ui/engine（否则分层倒置/环）
hit=$(grep -rnE '^[[:space:]]*use[[:space:]]+crate::(app|ui|engine)' src/model/ --include='*.rs' || true)
if [ -n "$hit" ]; then
  echo "[FAIL] model 层出现对 app/ui/engine 的依赖（内层不得依赖外层）:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] model 层零外层依赖"
fi

# 规则 2【依赖方向】engine 不得引用 app/ui（engine 是被 app 消费的低层机制）
hit=$(grep -rnE '^[[:space:]]*use[[:space:]]+crate::(app|ui)' src/engine/ --include='*.rs' || true)
if [ -n "$hit" ]; then
  echo "[FAIL] engine 层出现对 app/ui 的依赖:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] engine 层不依赖 app/ui"
fi

# 规则 3【依赖方向】ui 不得直接触 engine（展示层经 app 间接消费引擎）
hit=$(grep -rnE '^[[:space:]]*use[[:space:]]+crate::engine' src/ui/ --include='*.rs' || true)
if [ -n "$hit" ]; then
  echo "[FAIL] ui 层出现对 engine 的直接依赖（应经 app）:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] ui 层不直接依赖 engine"
fi

# 规则 4【入口边界】main.rs 不扩展应用主类型 impl（notes/rust.md 自动化检查条款：
# 入口特有逻辑归应用层模块，如 app/cli.rs）
hit=$(grep -nE '^impl App \{' src/main.rs || true)
if [ -n "$hit" ]; then
  echo "[FAIL] main.rs 内出现 impl App（入口不得扩展应用主类型）:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] main.rs 无 impl App 块"
fi

# 规则 5【适配方向】低层不得构造高层连接模型（notes/rust.md「低层模块不得构造
# 高层模块的展示类型」：违规形态即 LowView::to_high 式构造——engine 事件视图
# 只暴露纯数据，适配为 model::Connection 的动作只能发生在消费侧 app）。
# ui/model 处的构造不属此规则：ui → model 是合规外层方向，测试夹具直接构建
# 领域对象亦属正常（本规则只盯低层 engine）。
hit=$(grep -rnE 'Connection[[:space:]]*\{' src/engine/ --include='*.rs' \
      | grep -vE ':[0-9]+:[[:space:]]*//' || true)
if [ -n "$hit" ]; then
  echo "[FAIL] engine 层字面构造 model::Connection（适配应移消费侧 app）:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] engine 层不构造 Connection（纯数据视图，消费侧适配）"
fi

# 规则 6【单源】校验算法表唯一来源：CHECKSUM_ALGOS 只允许 model/checksum.rs 定义，
# 其他层只能 use/re-export（防双源漂移回归：app 层曾另有一份 2 元组同名表）
hit=$(grep -rnE '(pub )?const CHECKSUM_ALGOS' src/ --include='*.rs' | grep -v 'src/model/checksum.rs' || true)
if [ -n "$hit" ]; then
  echo "[FAIL] CHECKSUM_ALGOS 出现第二定义点（须单源于 model/checksum.rs）:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] CHECKSUM_ALGOS 单源 model/checksum.rs"
fi

# 规则 7【适配器边界】sentinel 终端适配器只允许入口引用：app/ui/engine/model
# 不得依赖 crate::sentinel（哨兵是入口特使的 IO 外壳，业务层触达即分层泄漏；
# architect 第二轮新增——v1.3 新增面 sentinel.rs 的边界规则化）
hit=$(grep -rnE 'crate::sentinel|sentinel::' src/app/ src/ui/ src/engine/ src/model/ \
      --include='*.rs' | grep -vE ':[0-9]+:[[:space:]]*//' || true)
if [ -n "$hit" ]; then
  echo "[FAIL] sentinel 适配器被业务层引用（只允许入口 main.rs）:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] sentinel 仅入口可见（业务层零依赖）"
fi

# 规则 8【环境变量收口】业务/机制层不直读环境变量：env 读取收口于适配缝
# （model/config.rs 路径重定位、sentinel.rs、入口 main.rs）；app/ui/engine 及
# model 其余模块出现 std::env::var(_os) 即违规（architect 第二轮新增）
hit=$(grep -rnE 'std::env::var(_os)?[[:space:]]*\(' src/app/ src/ui/ src/engine/ src/model/ \
      --include='*.rs' | grep -v 'src/model/config.rs' | grep -vE ':[0-9]+:[[:space:]]*//' || true)
if [ -n "$hit" ]; then
  echo "[FAIL] 业务/机制层直读环境变量（应收口 config/sentinel/入口）:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] 环境变量读取收口适配缝（config/sentinel/入口）"
fi

# 规则 9【窄接口】engine 实现子模块保持私有：supervisor/error/throttle 只允许
# 以 `mod` 声明（architect 第三轮：payload 类型经门面 re-export 消费；重新
# `pub mod` 即窄接口破坏——app/ui 将能 reach 引擎内部）
hit=$(grep -nE '^[[:space:]]*pub[[:space:]]+mod[[:space:]]+(supervisor|error|throttle)' src/engine/mod.rs || true)
if [ -n "$hit" ]; then
  echo "[FAIL] engine 实现子模块被 pub mod（窄接口破坏，应为私有 mod + 门面 re-export）:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] engine 实现子模块私有（窄接口经门面 re-export）"
fi

# 规则 10【单源】crate 级 lint 姿态单源于入口 main.rs：模块级姿态块（判别注释
# 「字节/速度/时间算术…」）不得再出现（architect 第三轮：曾 16 份拷贝致 CPD
# 重复，已收敛至 crate 根——再出现即重复姿态回归）
hit=$(grep -rln '字节/速度/时间算术在 u64-f64 间转换' src/ --include='*.rs' | grep -v '^src/main.rs$' || true)
if [ -n "$hit" ]; then
  echo "[FAIL] lint 姿态块出现第二拷贝（单源于 main.rs crate 级 allow）:"; echo "$hit"; fails=$((fails+1))
else
  echo "[ OK ] lint 姿态单源 main.rs（无模块级拷贝）"
fi

if [ "$fails" -gt 0 ]; then
  echo "arch_check: ${fails} 条规则未过"; exit 1
fi
echo "arch_check: 全部边界规则通过"

# ---- POSITIVE-CONTROL（一次性验证记录，非运行时步骤）------------------
# 首轮交付时已做阳性对照：向 src/engine/mod.rs 临时插入
#   `fn _probe() -> crate::model::Connection { crate::model::Connection { id: 0, start: 0, end: 0, done: 0 } }`
# 规则 5 即非零退出并命中该行；向 src/main.rs 临时插入 `impl App {}` 规则 4 命中；
# 向 src/ui/mod.rs 临时插入 `use crate::engine::EngineHandle;` 规则 3 命中；
# 对照后均已移除。零发现结论以阳性对照生效为前提（engineering.md）。
# architect 第二轮新增规则 7/8 阳性对照（本轮会话实测）：
#   向 src/app/mod.rs 临时插入 `use crate::sentinel::TerminalSentinel;`
#   → 规则 7 命中并非零退出；
#   向 src/engine/mod.rs 临时插入 `fn _probe_env() -> String { std::env::var("X").unwrap_or_default() }`
#   → 规则 8 命中并非零退出；对照后均已移除。
