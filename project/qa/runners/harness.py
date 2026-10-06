#!/usr/bin/env python3
"""harness.py — EZR QA 可执行套件核心基建（six-pack/QA 可执行化产物）。

职责（对应 qa/01-*-qa.md 各套件「环境前置」节）：
- EzrApp：伪终端驱动 ezr TUI（按键/鼠标注入、画面捕获、toast/列表/详情断言），
  参照 ezr-demo/scripts/capture.py 与 verify_ezr.py 的既定模式。
- Fixture：管理 ezr-fixture serve 子进程 + JSONL 请求日志解析（fixture 属 QA 工具入口，
  QA 规程明确允许）；请求日志断言用「包含口径」（engineering.md 测试执行纪律）。
- Case/Suite：P/F/S 记录、用例间环境隔离（独立 HOME + 独立保存目录）、汇总输出。
- 端到端口径：只经用户可达入口（TUI 按键/CLI 参数/磁盘产物）验证，不进入项目内部 API。

时序纪律（engineering.md）：
- 「门控 Given、武装 When」：fixture ?speed=B/s 使任务停在确定状态，供稳定观察窗。
- 正向探测加有界重试窗（wait_for 轮询至短超时）；负向探测单发（wait_gone）。
- 断言读实际渲染文本，归一化口径后再比较。
"""

from __future__ import annotations

import fcntl
import json
import os
import pty
import re
import select
import shutil
import signal
import struct
import subprocess
import tempfile
import termios
import time
import traceback
from dataclasses import dataclass, field

import pyte
import wcwidth


def _safe_display_rows(screen: "pyte.Screen") -> list[str]:
    """pyte Screen.display 的防御版渲染（逐格，不改变常规帧输出）。

    pyte 0.8.2 的 display 在特定帧（宽字符落在行末等边缘形态）会遇
    data=="" 的单元格并抛 IndexError（screens.py render: char[0]），
    令任意断言随机崩掉（实测 IC-02 中签）。断言只需文本内容：空数据格
    按空格渲染保持列对齐，宽字符 stub 跳过语义与 pyte 一致。
    """
    cols = screen.columns
    rows: list[str] = []
    for y in range(screen.lines):
        line = screen.buffer[y]
        parts: list[str] = []
        skip = False
        for x in range(cols):
            if skip:
                skip = False
                continue
            data = line[x].data
            if not data:
                parts.append(" ")
                continue
            if wcwidth.wcwidth(data[0]) == 2:
                skip = True
            parts.append(data)
        rows.append("".join(parts))
    return rows


class _ScreenProxy:
    """pyte Screen 薄代理：display 渲染失败时退化为 _safe_display_rows。

    所有经 app.screen.display / app.screen.buffer 的访问点（text/
    display_rows/find_row/detail_text/配色与柱条断言）统一获得防御。
    """

    def __init__(self, screen: "pyte.Screen"):
        self._screen = screen

    @property
    def display(self) -> list[str]:
        try:
            return self._screen.display
        except Exception:
            return _safe_display_rows(self._screen)

    def __getattr__(self, name: str):
        return getattr(self._screen, name)

# ---------------------------------------------------------------------------
# 路径常量（绝对路径：engineering.md「路径与工作目录确定性」）
# ---------------------------------------------------------------------------

QA_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_EZR = os.path.abspath(os.path.join(QA_DIR, "..", "..", "ezr"))
EZR_BIN = os.path.join(PROJECT_EZR, "target", "debug", "ezr")
FIXTURE_BIN = os.path.join(PROJECT_EZR, "target", "debug", "ezr-fixture")
PROXY_BIN = os.path.join(PROJECT_EZR, "target", "debug", "ezr-proxy")
TMP_ROOT = os.path.abspath(os.path.join(QA_DIR, "..", "..", "..", ".work", "tmp", "qa-run"))

MB = 1024 * 1024

# ---------------------------------------------------------------------------
# 按键编码（沿用 capture.py 契约）
# ---------------------------------------------------------------------------

NAMED_KEYS = {
    "up": b"\x1b[A", "down": b"\x1b[B", "right": b"\x1b[C", "left": b"\x1b[D",
    "enter": b"\r", "esc": b"\x1b", "tab": b"\t", "space": b" ",
    "pgup": b"\x1b[5~", "pgdn": b"\x1b[6~", "home": b"\x1b[H", "end": b"\x1b[F",
    "bs": b"\x7f", "ctrl_c": b"\x03",
}


def key_bytes(k: str) -> bytes:
    """键名 → 字节序列；mclick:列:行 等 SGR 鼠标事件；其余按字面文本。"""
    if k in NAMED_KEYS:
        return NAMED_KEYS[k]
    if k.startswith("mclick:"):
        _, x, y = k.split(":")
        return f"\x1b[<0;{x};{y}M".encode()
    if k.startswith("mup:"):
        _, x, y = k.split(":")
        return f"\x1b[<0;{x};{y}m".encode()
    if k.startswith("mwu:"):
        _, x, y = k.split(":")
        return f"\x1b[<64;{x};{y}M".encode()
    if k.startswith("mwd:"):
        _, x, y = k.split(":")
        return f"\x1b[<65;{x};{y}M".encode()
    return k.encode()


# ---------------------------------------------------------------------------
# fixture 管理
# ---------------------------------------------------------------------------


class Fixture:
    """ezr-fixture serve 包装：端口、根目录与 JSONL 日志三者独立可配。"""

    def __init__(self, port: int, root: str, log: str | None = None):
        self.port = port
        self.root = root
        self.log = log or os.path.join(root, f"access-{port}.jsonl")
        self.proc: subprocess.Popen | None = None

    def url(self, path: str) -> str:
        return f"http://127.0.0.1:{self.port}/{path}"

    def start(self) -> None:
        if self.proc is not None:
            return
        self.proc = subprocess.Popen(
            [FIXTURE_BIN, "serve", "--root", self.root, "--port", str(self.port),
             "--log", self.log],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        # 正向探测：有界重试窗等待端口就绪（engineering.md 正/负探测条款）
        import socket
        deadline = time.time() + 5.0
        while time.time() < deadline:
            try:
                with socket.create_connection(("127.0.0.1", self.port), timeout=0.2):
                    return
            except OSError:
                time.sleep(0.05)
        raise RuntimeError(f"fixture 端口 {self.port} 5s 未就绪")

    def requests(self) -> list[dict]:
        """读 JSONL 访问日志（全量行；日志为 append-only）。"""
        out = []
        try:
            with open(self.log, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line:
                        out.append(json.loads(line))
        except FileNotFoundError:
            pass
        return out

    def stop(self) -> None:
        if self.proc is not None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=3)
            self.proc = None


def gen_files(root: str) -> None:
    """生成 fixture 标准文件集（含 big-100m.bin 稀疏）。"""
    subprocess.run([FIXTURE_BIN, "gen", "--root", root], check=True,
                   stdout=subprocess.DEVNULL)


# ---------------------------------------------------------------------------
# QA 口径文件集（fixture 环境前置归 QA 套件所有）
#
# 口径决定（显式边界，随 handoff 落盘）：
# - 各名义尺寸文件按十进制生成（five-m.bin = 5,000,000 B …），使 QA 规程的
#   「详情显示 5.0 MB」（01-download-engine-15，UI 十进制显示为 demo 定稿基线）
#   与分块数断言（2.5MB→3 · 1MB→1 · 512KB→1 · 10MB→10，块大小 1 MiB）同时成立；
# - big-100m.bin = 104,857,600 B（规程唯一给出精确字节数的文件，AC-1/DE-16），
#   分块恰为 100 块；
# - 内容模式与 ezr-fixture gen 一致（i % 251），gen 对已存在文件跳过（不覆盖）。
# ---------------------------------------------------------------------------

QA_FILE_SIZES: dict[str, int] = {
    "five-m.bin": 5_000_000,
    "three-m.bin": 3_000_000,
    "eight-m.bin": 8_000_000,
    "twelve-m.bin": 12_000_000,
    "two-m.bin": 2_000_000,
    "one-m.bin": 1_000_000,
    "half-m.bin": 512_000,
    "ten-m.bin": 10_000_000,
    "small.bin": 1_024,
    "streamy.bin": 4_000_000,
    "two-half-m.bin": 2_500_000,
}


def gen_qa_files(root: str) -> None:
    """生成 QA 口径文件集：先写十进制名义尺寸文件，再由 fixture gen 补稀疏大文件。"""
    os.makedirs(root, exist_ok=True)
    for name, size in QA_FILE_SIZES.items():
        p = os.path.join(root, name)
        if os.path.exists(p):
            continue
        with open(p, "wb") as f:
            step = 64 * 1024
            written = 0
            while written < size:
                n = min(step, size - written)
                f.write(bytes((written + i) % 251 for i in range(n)))
                written += n
    gen_files(root)  # big-100m.bin(104,857,600 稀疏) + ghost-404.bin；已存在的跳过


# ---------------------------------------------------------------------------
# ezr TUI 伪终端驱动
# ---------------------------------------------------------------------------


class EzrApp:
    """在 PTY 中驱动 ezr TUI：按键注入 + pyte 画面捕获 + 进程生命周期管理。"""

    def __init__(self, home: str, args: list[str] | None = None, cols: int = 120,
                 rows: int = 34, env_extra: dict[str, str] | None = None,
                 save_dir: str | None = None):
        self.home = home
        self.cols, self.rows = cols, rows
        os.makedirs(home, exist_ok=True)
        env = {
            "HOME": home,
            "TERM": "xterm-256color",
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "LANG": "C.UTF-8",
            # 代理环境变量显式置空：ezr 不读环境变量代理（FR-01-61），
            # 置空防止本机其它代理注入干扰 loopback 请求
            "http_proxy": "", "https_proxy": "", "HTTP_PROXY": "", "HTTPS_PROXY": "",
            "NO_PROXY": "*",
        }
        if env_extra:
            env.update(env_extra)
        self.env = env
        argv = [EZR_BIN] + (args or [])
        self.save_dir = save_dir

        pid, fd = pty.fork()
        if pid == 0:  # 子进程：在 pty 中运行 ezr
            try:
                os.chdir(home)
                os.environ.update(env)
                if save_dir:
                    os.makedirs(save_dir, exist_ok=True)
                os.execvp(argv[0], argv)
            except Exception:
                os._exit(127)
        self.pid, self.fd = pid, fd
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        self._raw_screen = pyte.Screen(cols, rows)
        self.screen = _ScreenProxy(self._raw_screen)
        self.stream = pyte.ByteStream(self._raw_screen)
        self.exit_code: int | None = None
        self._pump(0.3)  # 首帧渲染等待

    # -- 输出泵 ---------------------------------------------------------------

    def pump(self, dur: float = 0.1) -> None:
        """把 PTY 输出喂给 pyte（时长 dur 秒）；进程退出时回收状态。"""
        self._pump(dur)

    def _pump(self, dur: float) -> None:
        deadline = time.time() + dur
        while time.time() < deadline:
            ready, _, _ = select.select([self.fd], [], [],
                                        min(0.05, max(0.0, deadline - time.time())))
            if self.fd not in ready:
                continue
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                data = b""
            if not data:
                self._reap()
                return
            self.stream.feed(data)

    def _reap(self) -> None:
        """子进程已退出：回收 exit code。"""
        if self.exit_code is None:
            try:
                wpid, status = os.waitpid(self.pid, os.WNOHANG)
                if wpid == self.pid:
                    self.exit_code = os.waitstatus_to_exitcode(status) if status else 0
            except ChildProcessError:
                pass

    # -- 输入 -----------------------------------------------------------------

    def send(self, *keys: str, gap: float = 0.06) -> None:
        """逐键注入（键间隔 gap 秒，给 TUI 事件循环处理窗口）。"""
        for k in keys:
            os.write(self.fd, key_bytes(k))
            self._pump(gap)

    def type_text(self, text: str) -> None:
        """整段文本注入（对话框字段输入）。"""
        for ch in text:
            os.write(self.fd, ch.encode())
            self._pump(0.01)

    # -- 画面观察 -------------------------------------------------------------

    def text(self) -> str:
        """当前画面纯文本（逐行 rstrip 后按行拼接）。"""
        return "\n".join(row.rstrip() for row in self.screen.display)

    def display_rows(self) -> list[str]:
        return [row.rstrip() for row in self.screen.display]

    def cell_fg(self, row: int, col: int) -> str | None:
        """(row, col) 单元格前景色名（配色断言用）。"""
        return self.screen.buffer[row][col].fg

    def find_row(self, needle: str) -> int | None:
        """第一个包含 needle 的行号（未命中 None）。"""
        for i, row in enumerate(self.screen.display):
            if needle in row:
                return i
        return None

    def wait_for(self, needle: str, timeout: float = 5.0) -> bool:
        """正向探测：轮询画面直至 needle 出现（有界重试窗）。"""
        deadline = time.time() + timeout
        while time.time() < deadline:
            self._pump(0.1)
            if needle in self.text():
                return True
        return False

    def wait_gone(self, needle: str, timeout: float = 5.0) -> bool:
        """负向探测：轮询画面直至 needle 消失。"""
        deadline = time.time() + timeout
        while time.time() < deadline:
            self._pump(0.1)
            if needle not in self.text():
                return True
        return False

    # -- 生命周期 -------------------------------------------------------------

    def graceful_quit(self, timeout: float = 6.0) -> int | None:
        """Q 优雅退出，等待进程结束并回收 exit code。"""
        os.write(self.fd, b"q")
        deadline = time.time() + timeout
        while time.time() < deadline:
            self._pump(0.1)
            self._reap()
            if self.exit_code is not None:
                break
        self.close()
        return self.exit_code

    def kill9(self) -> None:
        """SIGKILL 注入（崩溃恢复用例；无优雅退出、无钩子刷盘）。"""
        try:
            os.kill(self.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        try:
            os.waitpid(self.pid, 0)
        except ChildProcessError:
            pass
        self.close()

    def close(self) -> None:
        try:
            os.close(self.fd)
        except OSError:
            pass


# ---------------------------------------------------------------------------
# 磁盘产物断言助手
# ---------------------------------------------------------------------------


def read_file(path: str) -> bytes | None:
    try:
        with open(path, "rb") as f:
            return f.read()
    except OSError:
        return None


def file_bytes(path: str) -> int | None:
    try:
        return os.path.getsize(path)
    except OSError:
        return None


def expected_content(size: int) -> bytes:
    """fixture 文件内容确定性模式（i % 251，ezr-fixture.rs gen_files 口径）。"""
    return bytes(i % 251 for i in range(size))


def sha256_hex(data: bytes) -> str:
    import hashlib
    return hashlib.sha256(data).hexdigest()


def md5_hex(data: bytes) -> str:
    import hashlib
    return hashlib.md5(data).hexdigest()


def write_checksum_sidecar(save_dir: str, filename: str, algo: str, hexval: str) -> None:
    """预置校验伴随文件 <file>.<algo>（D14 后缀形态）。"""
    with open(os.path.join(save_dir, f"{filename}.{algo}"), "w", encoding="utf-8") as f:
        f.write(hexval)


# ---------------------------------------------------------------------------
# ezr-proxy 实例管理（跨套件共享：01-throttle-proxy / 03-modify-task / 02-named-proxy）
# ---------------------------------------------------------------------------


def start_proxy(env: "Env", name: str, port: int) -> dict:
    """启动 ezr-proxy 实例并返回句柄（进程 + 日志路径 + 端口）。

    日志落 env.home（用例隔离目录，随用例清理）。正向探测：端口就绪窗口。
    """
    log = os.path.join(env.home, f"{name}.jsonl")
    proc = subprocess.Popen(
        [PROXY_BIN, "serve", "--port", str(port), "--name", name, "--log", log],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    import socket
    deadline = time.time() + 5.0
    while time.time() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                break
        except OSError:
            time.sleep(0.05)
    else:
        proc.kill()
        raise RuntimeError(f"ezr-proxy {name} 端口 {port} 5s 未就绪")
    return {"proc": proc, "log": log, "port": port, "name": name}


def proxy_reqs(handle: dict) -> list[dict]:
    """读代理 JSONL 日志（全量行；append-only，包含口径断言用）。"""
    out: list[dict] = []
    try:
        with open(handle["log"], encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    out.append(json.loads(line))
    except (OSError, json.JSONDecodeError):
        pass
    return out


def stop_proxy(handle: dict) -> None:
    handle["proc"].terminate()
    try:
        handle["proc"].wait(timeout=3)
    except subprocess.TimeoutExpired:
        handle["proc"].kill()
        handle["proc"].wait(timeout=3)


# ---------------------------------------------------------------------------
# 用例与套件
# ---------------------------------------------------------------------------


class SkipCase(Exception):
    """用例内动态判定环境不满足 → S（内嵌环境判据）。"""


@dataclass
class CaseResult:
    case_id: str
    status: str  # P / F / S
    note: str = ""


@dataclass
class Env:
    """每个用例的隔离环境（独立 HOME / 保存目录 / fixture 根目录）。"""
    home: str
    save_dir: str
    fixture_root: str
    fixture: Fixture
    extra: dict = field(default_factory=dict)

    def config_path(self) -> str:
        d = os.path.join(self.home, ".ezr")
        os.makedirs(d, exist_ok=True)
        return os.path.join(d, "config.toml")

    def write_config(self, body: str) -> None:
        with open(self.config_path(), "w", encoding="utf-8") as f:
            f.write(body)


class Suite:
    """套件基类：case 注册、隔离环境、P/F/S 记录与汇总。"""

    def __init__(self, name: str, port_base: int):
        self.name = name
        self.port_base = port_base
        self._port = port_base
        self.results: list[CaseResult] = []
        os.makedirs(TMP_ROOT, exist_ok=True)

    def case(self, case_id: str):
        """装饰器：注册用例函数；函数签名 (self, env)。"""
        def deco(fn):
            self._run_case(case_id, fn)
            return fn
        return deco

    def skip(self, case_id: str, reason: str) -> None:
        """登记受限项（S 内嵌环境判据）。"""
        self.results.append(CaseResult(case_id, "S", reason))

    def _next_port(self) -> int:
        self._port += 1
        return self._port

    def _run_case(self, case_id: str, fn) -> None:
        env_home = tempfile.mkdtemp(prefix="qa-home-", dir=TMP_ROOT)
        save_dir = tempfile.mkdtemp(prefix="qa-dl-", dir=TMP_ROOT)
        fixture_root = tempfile.mkdtemp(prefix="qa-fix-", dir=TMP_ROOT)
        gen_qa_files(fixture_root)
        fixture = Fixture(self._next_port(), fixture_root)
        env = Env(home=env_home, save_dir=save_dir, fixture_root=fixture_root,
                  fixture=fixture)
        t0 = time.time()
        try:
            fixture.start()
            fn(env)
            self.results.append(CaseResult(case_id, "P"))
            print(f"  [{self.name}] {case_id}: P ({time.time()-t0:.1f}s)")
        except SkipCase as e:
            self.results.append(CaseResult(case_id, "S", str(e)))
            print(f"  [{self.name}] {case_id}: S ({e})")
        except AssertionError as e:
            self._snapshot(env, case_id)
            self.results.append(CaseResult(case_id, "F", str(e)))
            print(f"  [{self.name}] {case_id}: F — {str(e)[:400]}")
        except Exception as e:  # 环境性异常按 F 记录并附 traceback
            self._snapshot(env, case_id)
            self.results.append(CaseResult(case_id, "F", f"EXC {type(e).__name__}: {e}"))
            traceback.print_exc()
            print(f"  [{self.name}] {case_id}: F (EXC {type(e).__name__}: {e})")
        finally:
            fixture.stop()
            shutil.rmtree(env_home, ignore_errors=True)
            shutil.rmtree(save_dir, ignore_errors=True)
            shutil.rmtree(fixture_root, ignore_errors=True)

    def _snapshot(self, env: Env, case_id: str) -> None:
        """失败现场快照钩子（由子类覆写：捕获当前画面）。"""
        pass

    def run(self) -> list[CaseResult]:
        raise NotImplementedError

    def summary(self) -> tuple[int, int, int]:
        p = sum(1 for r in self.results if r.status == "P")
        f = sum(1 for r in self.results if r.status == "F")
        s = sum(1 for r in self.results if r.status == "S")
        return p, f, s


# ---------------------------------------------------------------------------
# 跨套件共享操作助手（TUI 用户可达路径封装）
# ---------------------------------------------------------------------------


def add_task_via_dialog(app: "EzrApp", env: "Env", url: str, conns: str = "",
                        ck_value: str = "", save_dir: str | None = None) -> None:
    """A 对话框添加任务：URL + 保存目录 +（可选并发/校验码）→ Enter 确认。

    save_dir 缺省打 env.save_dir；需要目录隔离的用例（同 case 多轮下载、
    避免落盘脏测与重复守卫）显式传独立子目录。
    """
    app.send("A")
    assert app.wait_for("添加下载任务", 3), "添加对话框未打开"
    app.type_text(url)
    app.send("tab")
    # 哨兵语义：None=env.save_dir；""=字段留空（走默认 download_dir）；
    # "路径"=显式目录。不可用 or（空串会被吞掉）。
    app.type_text(env.save_dir if save_dir is None else save_dir)
    # 字段序：URL → 保存到 → 并发 → 校验（下拉）→ 校验码
    if conns:
        app.send("tab")
        app.type_text(conns)
        tabs_to_ck = 2  # 当前在并发：校验、校验码
    else:
        tabs_to_ck = 3  # 当前在保存到：并发、校验、校验码
    if ck_value:
        app.send(*(["tab"] * tabs_to_ck))
        app.type_text(ck_value)
    app.send("enter")
    app.pump(0.3)


def detail_text(app: "EzrApp") -> str:
    """任务详情面板箱体文本（120 列布局右栏，不含下方「全局速度」图表面板）。

    区域锚定（engineering.md）：右栏自上而下 = 详情面板 + 图表面板，图表面板
    标题「↓ 全局速度 …」含「速度」子串——整右栏截取会把它误算进详情断言区
    （v1.11 起规格只约束详情面板无「状态/速度」字段行，图表面板合法常驻）。
    故以「任务详情」标题行起、图表面板标题行止（不含）截取箱体行。
    """
    rows = app.screen.display
    top = next((i for i, r in enumerate(rows) if "任务详情" in r), None)
    if top is None:
        return ""
    end = next(
        (i for i in range(top + 1, len(rows)) if "全局速度" in rows[i]), len(rows)
    )
    return "\n".join(row[64:].rstrip() for row in rows[top:end])


def task_pct(app: "EzrApp") -> float | None:
    r"""选中任务行的进度百分比（无选中行时退回全画面首个匹配）。

    区域锚定（engineering.md）：进度百分比渲染在列表任务行右缘
    （实测列 60–65），右栏切片（detail_text，列 64 起）永远读不全
    （截断片段不匹配 ``\d+\.\d%``）——必须读全宽行。选中行以光标
    ▌ 标记（task_lines 渲染，单元测试锁定）；无选中行时退回全画面
    首个匹配（单任务场景等价）。
    """
    rows = app.screen.display
    sel = [r for r in rows if "▌" in r]
    for r in (sel if sel else rows):
        m = re.search(r"(\d+\.\d)%", r)
        if m:
            return float(m.group(1))
    return None


def select_completed(app: "EzrApp", needles: list[str],
                     max_moves: int = 8) -> set[str]:
    """Tab 切「已完成」页签，Home 起步逐个选中直至断言集合全部命中。"""
    app.send("tab")
    app.pump(0.4)
    app.send("home")
    app.pump(0.3)
    found: set[str] = set()
    deadline = time.time() + 12
    moves = 0
    while time.time() < deadline and len(found) < len(needles) and moves <= max_moves:
        app.pump(0.3)
        d = detail_text(app)
        for n in needles:
            if n in d:
                found.add(n)
        if len(found) < len(needles):
            app.send("down")
            moves += 1
    return found


# ---------------------------------------------------------------------------
# 断言助手
# ---------------------------------------------------------------------------


def assert_in(needle: str, hay: str, what: str = "") -> None:
    if needle not in hay:
        raise AssertionError(
            f"画面缺少「{needle}」{('（' + what + '）') if what else ''}\n--- 画面 ---\n{hay[:2000]}")


def assert_not_in(needle: str, hay: str, what: str = "") -> None:
    if needle in hay:
        raise AssertionError(
            f"画面不应出现「{needle}」{('（' + what + '）') if what else ''}\n--- 画面 ---\n{hay[:2000]}")


def parse_speed(bps_text: str) -> float:
    """「1.02 MB/s」/「860 KB/s」→ 字节/秒。"""
    t = bps_text.strip()
    if t.endswith("MB/s"):
        return float(t[:-4]) * MB
    if t.endswith("KB/s"):
        return float(t[:-4]) * 1000.0
    if t.endswith("B/s"):
        return float(t[:-3])
    raise ValueError(f"无法解析速度读数: {bps_text!r}")


def assert_file_content(path: str, size: int) -> None:
    """磁盘产物字节完整性（fixture 内容模式 i%251）。"""
    data = read_file(path)
    assert data is not None, f"目标文件不存在: {path}"
    assert len(data) == size, f"文件大小 {len(data)} ≠ 预期 {size}"
    if size <= 16 * MB:
        assert data == expected_content(size), f"文件内容与 fixture 模式不符: {path}"


def wait_file_size(path: str, size: int, timeout: float = 30.0,
                   app: "EzrApp | None" = None) -> None:
    """轮询文件直至达到 size（完成落盘正探测）。

    app 参数：轮询期间持续泵 PTY——TUI 以 100ms tick 重绘，阻塞式轮询会让
    PTY 输出缓冲区塞满、应用写屏阻塞乃至整个下载停摆（会话复盘教训）。
    """
    deadline = time.time() + timeout
    while time.time() < deadline:
        if app is not None:
            app.pump(0.05)
        if file_bytes(path) == size:
            return
        time.sleep(0.05)
    raise AssertionError(
        f"{timeout}s 内文件未达到 {size} 字节（当前 {file_bytes(path)}）: {path}")
