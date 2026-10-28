# 测试经验

## proptest 随机种子敏感：f64 守恒类断言写相对容差
- **现象**：`prop_throttle_bucket_step_invariants` 在无任何相关代码改动的情况下突然失败
  （预算守恒 1-ULP 差：`487326933.1084516` vs `487326933.10845166`），复跑又时过时不过。
- **处置**：把 `taken + tokens_after == available` 改为相对容差断言
  `|taken + tokens_after - available| <= |available| * 1e-9`，复跑 3 次稳定绿。
- **根因**：proptest 每次运行随机种子，新增测试会改变进程内用例执行序与 RNG 流，命中
  此前从未抽到的极端输入（`remaining/available ≈ 0.448` 不满足 Sterbenz 引理适用域，
  `clamped - taken` 舍入一轮）；f64 精确相等守恒在极端量级下本就不可满足。回归基线 0F
  结论要意识到属性测试的种子敏感性——新增/删除测试可能"翻转"既有属性测试的命中面。

## BT 路由测试夹具：种子文件走 testenv 唯一目录
- 写 BT 启动路由类单测时，.torrent 夹具用 `crate::model::testenv::uniq_tmp_dir` 建唯一
  目录落盘（并发套件收编时避免路径碰撞），用例结束 `remove_dir_all` 清理；引擎 spy 用
  `EngineHandle::from_sender`（cfg(test) 构造器）替换 `app.engine` 断言收到的 Cmd。
