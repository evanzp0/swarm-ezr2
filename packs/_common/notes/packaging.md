# 打包与交付专项注意事项

> 归属规则：跨语言/跨工具通则见 `packs/_common/engineering.md`；本文件只收 swarm
> 打包交付（`bin/swarm complete` 归档、交付 tarball 的消费）与 git/tar 工具相关的
> 专项问题与解法（会话复盘沉淀，操作者维护）。

## bin/swarm complete 归档

- **打包清单来自 git 索引，收尾前先 `git add -A`**：`bin/swarm complete` 用
  `git ls-files --cached --others --exclude-standard` 生成清单。磁盘已删除但仍在
  索引中的路径（`rm` 未 `git rm`）会让 tar 因清单路径不存在而失败——会话内做过
  文件删除/重组的角色，收尾前先 `git add -A` 刷新索引再 complete。两点口径注意：
  新建文件即使未 `add` 也会经 `--others` 进包；被 `.gitignore` 匹配的新文件会被
  `--exclude-standard` **静默排除**——需要交付的文件勿依赖 gitignore 路径存放。

## 交付 tarball 的消费

- **覆盖解压到既有树会留「过期文件」**：tar 无删除语义——上游做过目录式重组
  （如 `a.rs` → `a/`）时，下游在旧 clone 上覆盖解压会得到 `a.rs` 与 `a/mod.rs`
  并存，编译报模块歧义（且冲突不止编译器点名的那几个，凡「origin 有、包内无」
  的路径全是雷）。解压前先 diff 双方文件清单（`tar -tzf pkg.tar.gz | sort` vs
  `git ls-files | sort`），把清单差集里的过期文件先删再解压；或干脆 fresh clone
  后整树替换。
