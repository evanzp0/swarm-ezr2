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

import base64
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
# ezr-proxy 内嵌自签证书的 PEM 形态（v140 扩展）：供 SSL_CERT_FILE 注入产品进程
# （reqwest rustls-tls-native-roots 经 rustls-native-certs 0.8 / openssl-probe
# 读取该环境变量；SAN：IP 127.0.0.1 / IP ::1 / DNS localhost）
PROXY_CA_PEM = os.path.join(PROJECT_EZR, "src", "bin", "ezr-proxy-certs", "ca.pem")
TMP_ROOT = os.path.abspath(os.path.join(QA_DIR, "..", "..", "..", ".work", "tmp", "qa-run"))


def _ensure_proxy_ca_pem() -> None:
    """保证 PROXY_CA_PEM 在位：缺失时由内嵌证书 server.der 派生（fixture 自足）。

    ca.pem 是 ezr-proxy 内嵌自签证书（server.der）的 PEM 形态（同名同义，
    规程 02-named-proxy-qa.md 环境前置与 ezr-proxy.rs 模块注释声明口径）——
    派生文件不入库，fresh 环境重建工作区后缺失，https 相位用例会因产品
    不信任桩证书而全数失败。此处按 create-new 语义补齐（已存在不重写），
    派生是 DER→PEM 的无损编码转换，信任锚与桩实际出示证书逐字节一致。
    server.der 也缺失时直接报错（fail-loud，不静默降级）。
    """
    if os.path.exists(PROXY_CA_PEM):
        return
    der_path = os.path.join(os.path.dirname(PROXY_CA_PEM), "server.der")
    der = open(der_path, "rb").read()  # 缺失即抛错：证书是桩的组成部分
    b64 = base64.b64encode(der).decode("ascii")
    lines = "\n".join(b64[i:i + 64] for i in range(0, len(b64), 64))
    pem = f"-----BEGIN CERTIFICATE-----\n{lines}\n-----END CERTIFICATE-----\n"
    with open(PROXY_CA_PEM, "x", encoding="ascii") as f:
        f.write(pem)


_ensure_proxy_ca_pem()

MB = 1024 * 1024

# ---------------------------------------------------------------------------
# 按键编码（沿用 capture.py 契约）
# ---------------------------------------------------------------------------

NAMED_KEYS = {
    "up": b"\x1b[A", "down": b"\x1b[B", "right": b"\x1b[C", "left": b"\x1b[D",
    "enter": b"\r", "esc": b"\x1b", "tab": b"\t", "space": b" ",
    "pgup": b"\x1b[5~", "pgdn": b"\x1b[6~", "home": b"\x1b[H", "end": b"\x1b[F",
    "bs": b"\x7f", "ctrl_c": b"\x03", "backtab": b"\x1b[Z",
    # Ctrl 组合键（04-ui-conns 并发明细交互，FR-01-97：Ctrl+↑↓ 选择、Ctrl+B 断开）
    "ctrl_up": b"\x1b[1;5A", "ctrl_down": b"\x1b[1;5B",
    "ctrl_b": b"\x02",
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
    if k.startswith("mdrag:"):
        # 左键按住移动（SGR motion，button 32）：窗口/滑块拖拽的随动事件
        _, x, y = k.split(":")
        return f"\x1b[<32;{x};{y}M".encode()
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
        # 原始输出尾流（有界）：OSC 52 剪贴板序列断言用（04-ui-conns-18；
        # 产品无图形会话回退 OSC 52，序列被 pyte 吞掉，只能从原始字节流捕获）
        self._raw_tail = bytearray()
        self._raw_tail_cap = 262144
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
            # 原始尾流有界追加（OSC 52 捕获；超限丢弃最老半量，摊还 O(1)）
            self._raw_tail.extend(data)
            if len(self._raw_tail) > self._raw_tail_cap:
                del self._raw_tail[: self._raw_tail_cap // 2]

    def osc52_payloads(self) -> list[str]:
        """自原始尾流提取全部 OSC 52 剪贴板载荷（base64 解码，按序去重相邻重复）。"""
        import base64
        out: list[str] = []
        for m in re.finditer(b"\\x1b]52;c;([A-Za-z0-9+/=]*)\x07", bytes(self._raw_tail)):
            try:
                out.append(base64.b64decode(m.group(1)).decode("utf-8", "replace"))
            except Exception:
                continue
        dedup: list[str] = []
        for v in out:
            if not dedup or dedup[-1] != v:
                dedup.append(v)
        return dedup

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

    # -- 终端尺寸 -------------------------------------------------------------

    def resize(self, cols: int, rows: int) -> None:
        """PTY 窗口尺寸变更（ioctl + pyte 同步 resize）。

        pyte Screen 不随 PTY 自动缩放：只发 ioctl 会让 buffer 停留在旧宽度，
        列空间断言（分隔符/图区定位）会读到旧帧残留列——必须同步 resize。
        """
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        self._raw_screen.resize(rows, cols)
        self.cols, self.rows = cols, rows

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


def start_proxy(env: "Env", name: str, port: int, *, scheme: str | None = None,
                user: str | None = None, password: str | None = None,
                bind: str | None = None) -> dict:
    """启动 ezr-proxy 实例并返回句柄（进程 + 日志路径 + 端口）。

    v140 扩展：scheme（http 缺省 / https 代理自身 TLS / socks5+RFC 1929）、
    user/password（http/https 认证强制模式；socks5 凭证必填）、bind（缺省
    127.0.0.1，IPv6 回环传 "::1"）。缺省形态与旧版逐参数兼容。
    日志落 env.home（用例隔离目录，随用例清理）。正向探测：端口就绪窗口
    （探活按 bind 地址连接；TLS/socks5 形态下裸 TCP 连接即达标，桩容忍探活
    连接即断）。
    """
    log = os.path.join(env.home, f"{name}.jsonl")
    args = [PROXY_BIN, "serve", "--port", str(port), "--name", name, "--log", log]
    if scheme:
        args += ["--scheme", scheme]
    if user is not None:
        args += ["--user", user]
    if password is not None:
        args += ["--password", password]
    if bind:
        args += ["--bind", bind]
    proc = subprocess.Popen(args,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    import socket
    probe_host = bind or "127.0.0.1"
    deadline = time.time() + 5.0
    while time.time() < deadline:
        try:
            with socket.create_connection((probe_host, port), timeout=0.2):
                break
        except OSError:
            time.sleep(0.05)
    else:
        proc.kill()
        raise RuntimeError(f"ezr-proxy {name} 端口 {port} 5s 未就绪")
    return {"proc": proc, "log": log, "port": port, "name": name,
            "scheme": scheme or "http", "bind": bind or "127.0.0.1"}


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


DUMMY_URL = "http://qa.local/probe.bin"


def open_add_dialog_via_cmd(app: "EzrApp", url: str) -> None:
    """A → 命令窗口输入 URL → Enter 分流打开「添加任务：HTTP(S)」。

    06-cmd-window-02 口径（phase-02 起添加任务唯一入口）：http(s) URL →
    HTTP 对话框且 URL 字段预填、并发自动回填 4；torrent 路径 → BT 对话框；
    其余输入 → 错误窗口。命令窗口打开锚 = 标题「请输入命令或链接」。
    """
    app.send("A")
    assert app.wait_for("请输入命令或链接", 3), "命令窗口未打开"
    app.type_text(url)
    app.send("enter")
    assert app.wait_for("添加任务：HTTP(S)", 3), "添加对话框未打开"


def add_task_via_dialog(app: "EzrApp", env: "Env", url: str, conns: str = "",
                        ck_value: str = "", save_dir: str | None = None) -> None:
    """A 对话框添加任务：URL + 保存目录 +（可选并发/校验码）→ Enter 确认。

    save_dir 缺省打 env.save_dir；需要目录隔离的用例（同 case 多轮下载、
    避免落盘脏测与重复守卫）显式传独立子目录。
    """
    open_add_dialog_via_cmd(app, url)
    app.send("tab")
    # 哨兵语义：None=env.save_dir；""=字段留空（走默认 download_dir）；
    # "路径"=显式目录。不可用 or（空串会被吞掉）。
    app.type_text(env.save_dir if save_dir is None else save_dir)
    # 字段序：URL → 保存到 → 并发 → 校验（下拉）→ 校验码
    if conns:
        app.send("tab")
        # v1.30/FR-01-01：URL 输入自动回填默认并发（未手动改过时）——
        # 先退格清空回填值（≤2 位）再输入目标并发，防 "4"+"3"→"43" 拼接
        app.send("bs", "bs")
        app.type_text(conns)
        tabs_to_ck = 2  # 当前在并发：校验、校验码
    else:
        tabs_to_ck = 3  # 当前在保存到：并发、校验、校验码
    if ck_value:
        # v1.32/FR-01-01：校验下拉默认 NONE，NONE 选中时校验码实时清空且
        # 不可输入（逐键/粘贴同拦截）——先在校验字段展开下拉选 SHA-256
        # （↓×4 自 NONE 起选中即改值），Tab 关下拉并跳校验码字段再输码
        app.send(*(["tab"] * (tabs_to_ck - 1)))  # 落在校验字段
        app.send("enter")                        # 展开下拉（选中即改值）
        app.send(*(["down"] * 4))                # NONE→MD5→SHA-1→SHA-224→SHA-256
        app.send("tab")                          # 关下拉，焦点跳校验码
        app.type_text(ck_value)
    app.send("enter")
    app.pump(0.3)


def detail_text(app: "EzrApp") -> str:
    """任务详情面板箱体文本（120 列布局右栏，不含下方「并发连接」面板）。

    区域锚定（engineering.md「not_in 子串断言的区域必须与规格语义面板对齐」）：
    v1.13/FR-01-94 定稿布局右栏自上而下 = 任务详情面板 + 并发连接面板
    （详情正下方占右列余高；v1.13 前的「↓ 全局速度」独立图表面板已撤销，
    流量图内嵌头部右侧且无标题）。详情语义区域 = 「任务详情」标题行起、
    「并发连接」标题行止（不含）——截取到屏幕底会混入连接面板的
    「下载速度」列头等合法常驻文本，使 TD-03 的 not_in「速度」误报。
    """
    rows = app.screen.display
    top = next((i for i, r in enumerate(rows) if "任务详情" in r), None)
    if top is None:
        return ""
    end = next(
        (i for i in range(top + 1, len(rows)) if "并发连接" in rows[i]), len(rows)
    )
    return "\n".join(row[64:].rstrip() for row in rows[top:end])


def chart_points(app: "EzrApp") -> int:
    """头部右侧内嵌流量图的采样点数（1 点 = 1 列含柱条字符，按列去重）。

    v1.13/FR-01-94 口径：流量图无边框无标题、占头部内容区全高（第 1–3 行）、
    宽为内容区 20%，区域 = 每行最右两个「│」之间（字段区结束分隔符与
    面板右边框）。按列去重而非字符计数——纵向柱高随数值幅度变化，
    字符总数度量的是幅度和而非点数（engineering.md「画面字符计数
    先限区域再按列去重」）。速度为零时图区无柱条字符，返回 0。
    """
    rows = app.screen.display
    cols: set[int] = set()
    for row in rows[1:4]:
        right = row.rfind("│")
        if right < 0:
            continue
        left = row.rfind("│", 0, right)
        if left < 0:
            continue
        for x, ch in enumerate(row[left + 1:right]):
            if ch in "▁▂▃▄▅▆▇█":
                cols.add(x)
    return len(cols)


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


def conns_badge(app: "EzrApp") -> "tuple[int, int] | None":
    """并发连接面板标题徽标 (x, y)（v1.34/FR-01-96：` x/y 项 `——x = 活跃
    明细行数（正在连接 + 已连接，与明细严格同源）、y = 任务最大并发数；
    面板未渲染返回 None）。

    区域锚定（engineering.md「not_in 子串断言的区域必须与规格语义面板
    对齐」）：「下载任务」面板标题徽标同为 `` x/y 项 `` 形态——必须锚定
    「并发连接」标题行，全画面正则会撞列表面板计数。右栏 buffer 列空间
    读取（左栏 CJK 宽字符会使 display 串列位漂移，同 detail_text 区域
    纪律；跳过宽字符续位空格 data==""）。
    """
    for i in range(app.screen.lines):
        right = "".join(
            app.screen.buffer[i][x].data or ""
            for x in range(64, app.screen.columns)
        )
        if "并发连接" not in right:
            continue
        m = re.search(r"(\d+)/(\d+) 项", right)
        if m:
            return int(m.group(1)), int(m.group(2))
        return None
    return None


def select_completed(app: "EzrApp", needles: list[str],
                     max_moves: int = 8) -> set[str]:
    """] 切「已完成」页签（v1.30：Tab 改面板焦点，页签键 ]/[/T 承载），
    Home 起步逐个选中直至断言集合全部命中。"""
    app.send("]")
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
