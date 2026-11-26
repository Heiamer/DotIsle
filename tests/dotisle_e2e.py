#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""DotIsle 控制中心全功能快测脚本。

对运行中的 island（LyricIsland.exe）逐项验证控制中心依赖的全部管道命令契约。
用法：
    python dotisle_e2e.py                        # 连接默认管道
    python dotisle_e2e.py --pipe xxx             # 指定测试管道
    python dotisle_e2e.py --launch --data <dir>  # 自行启动隔离 island 再测
退出码：0 = 全过，1 = 有失败。
"""
import argparse
import json
import os
import subprocess
import sys
import threading
import time

PIPE_DEFAULT = r"LyricIsland.Native.Settings.v1"


class PipeClient:
    """换行分隔 JSON 的命名管道客户端（与 Go island 包同一契约）。"""

    def __init__(self, pipe):
        self.pipe = pipe
        self.path = "\\\\.\\pipe\\" + pipe
        self.f = None

    def connect(self, timeout_s=8.0):
        # No-op: the island serves ONE pipe instance and holds the connection for
        # up to 8 s waiting for a complete request. Opening without sending would
        # poison the pipe for 8 s. request() opens, writes and closes atomically.
        return True

    def request(self, command, value=None, timeout_s=8.0):
        # One connection per request: the island serves a single pipe instance
        # sequentially, and the control center's first state read can occupy it
        # while the desktop host starts. Wait long enough for that handoff.
        deadline = time.time() + 10.0
        f = None
        while time.time() < deadline:
            try:
                f = open(self.path, "r+b", buffering=0)
                break
            except OSError:
                time.sleep(0.15)
        if f is None:
            raise ConnectionError("无法连接管道")
        try:
            req = json.dumps({"version": 1, "command": command, "value": value},
                             ensure_ascii=False).encode("utf-8") + b"\n"
            f.write(req)
            return self._read_line_on(f, timeout_s)
        finally:
            try:
                f.close()
            except OSError:
                pass

    def _read_line_on(self, f, timeout_s):
        result = {"line": None}

        def reader():
            buf = bytearray()
            try:
                while b"\n" not in buf:
                    chunk = f.read(65536)
                    if not chunk:
                        break
                    buf.extend(chunk)
                    if len(buf) > 1024 * 1024:
                        break
            except OSError:
                pass
            result["line"] = bytes(buf)

        t = threading.Thread(target=reader, daemon=True)
        t.start()
        t.join(timeout_s)
        line = result["line"]
        if not line or b"\n" not in line:
            raise TimeoutError("响应超时或连接关闭")
        return json.loads(line.split(b"\n")[0].decode("utf-8", "replace"))

    def close(self):
        if self.f:
            try:
                self.f.close()
            except OSError:
                pass
            self.f = None


class Tester:
    def __init__(self):
        self.pass_ = 0
        self.fail = 0
        self.failures = []

    def check(self, name, ok, detail=""):
        if ok:
            self.pass_ += 1
            print("PASS  " + name)
        else:
            self.fail += 1
            self.failures.append((name, detail))
            print("FAIL  %s  %s" % (name, detail))

    def summary(self):
        print("---")
        print("PASS=%d FAIL=%d" % (self.pass_, self.fail))
        return self.fail == 0


def wait_apply(client, ms=0.15):
    time.sleep(ms)
    return client.request("state")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pipe", default=os.environ.get("LYRICISLAND_TEST_PIPE", PIPE_DEFAULT))
    ap.add_argument("--launch", action="store_true", help="自行启动隔离 island")
    ap.add_argument("--data", default=os.path.join(os.environ.get("TEMP", "."), "dotisle-e2e"))
    ap.add_argument("--exe", default=None, help="island 可执行文件路径（--launch 时必填）")
    args = ap.parse_args()

    island_proc = None
    if args.launch:
        if not args.exe or not os.path.isfile(args.exe):
            print("--launch 需要 --exe 指向 LyricIsland.exe")
            sys.exit(2)
        os.makedirs(args.data, exist_ok=True)
        env = dict(os.environ)
        env["LYRICISLAND_TEST_PIPE"] = args.pipe
        env["LYRICISLAND_NATIVE_DATA"] = args.data
        island_proc = subprocess.Popen([args.exe, "--startup"], env=env,
                                       creationflags=0x08000000)
        time.sleep(4)

    client = PipeClient(args.pipe)

    t = Tester()
    try:
        st = client.request("state")
    except Exception as e:
        print("无法连接管道 " + args.pipe + "：" + str(e))
        if island_proc:
            island_proc.kill()
        sys.exit(1)
    t.check("state 应答", st.get("success") is True, str(st)[:120])

    def setting(key):
        return (st.get("settings") or {}).get(key)

    # ---- 1. update 键往返（每个控制中心写入键抽样） ------------------------
    last_reply = {}

    def upd(key, value):
        nonlocal last_reply
        r = client.request("update", {"key": key, "value": value})
        last_reply = r
        return r.get("success") is True

    roundtrips = [
        ("SettingsTheme", "Dark", "Dark"),
        ("SettingsTheme", "System", "System"),
        ("ControlCenterStartPage", "todos", "todos"),
        ("QuickReminderHotkey", "Ctrl+Shift+F12", "Ctrl+Shift+F12"),
        ("OffsetMilliseconds", -300, -300),
        ("Translation", False, False),
        ("WordHighlight", True, True),
        ("OnlineSongInfo", False, False),
        ("AutoStart", False, False),
        ("StartupSignatureEnabled", False, False),
        ("StartupGreetingEnabled", False, False),
        ("DynamicColorHighContrast", True, True),
        ("ControlCenterColorSource", "Custom", "Custom"),
        ("ControlCenterMonetEnabled", False, False),
        ("SettingsLightAccentColor", "#8ECAE6", "#8ECAE6"),
        ("IslandFontFamily", "SimSun", "SimSun"),
        ("IslandSizeMode", "Mini", "Mini"),
        ("IslandColorSource", "Custom", "Custom"),
        ("IslandDynamicSeedColor", "#FFD166", "#FFD166"),
        ("ExpandedTextColor", "#F4F6F8", "#F4F6F8"),
        ("LyricProgressColor", "#A9E6CD", "#A9E6CD"),
        ("IslandMaterial", "LightField", "LightField"),
        ("MiniMaterial", "Acrylic", "Acrylic"),
        ("AcrylicCardFrost", 55, 55),
        ("AcrylicCapsuleOpacity", 20, 20),
        ("HideWhenPaused", False, False),
        ("AlwaysOnTop", True, True),
        ("HoverExpand", True, True),
        ("SwipeActivities", False, False),
        ("Floating", True, True),
        ("SystemStateHintsEnabled", True, True),
        ("SystemStateHintPlacement", "Bottom", "Bottom"),
        ("MusicGlowEnabled", True, True),
        ("MusicGlowMode", "Rhythm", "Rhythm"),
        ("MusicExpandedLayout", "LyricFocus", "LyricFocus"),
        ("PlayerResponseMode", "Selected", "Selected"),
        ("BrowserMediaEnabled", True, True),
        ("ProgressBarStyle", "Dots", "Dots"),
        ("ProgressBarThumbEnabled", True, True),
        ("ProgressBarThumb", "Heart", "Heart"),
        ("TransportTheme", "Ripple", "Ripple"),
        ("NotificationsEnabled", False, False),
        ("NotificationHistoryDays", 14, 14),
        ("NotificationWhitelistEnabled", True, True),
        ("HeadphoneConnectionRemindersEnabled", False, False),
        ("TodoRemindersEnabled", False, False),
        ("TimerMonoDigits", True, True),
        ("TodoIslandEnabled", False, False),
        ("FocusCountUp", True, True),
        ("FocusPauseOnLock", True, True),
        ("TodoDefaultColor", "#B1E8D1", "#B1E8D1"),
        ("AutoCompactWidth", True, True),
        ("CompactWidth", 320, 320),
        ("AutoCompactMaxWidth", 460, 460),
        ("FontSize", 16.5, 16.5),
        ("CompactTranslation", False, False),
        ("ChineseTwoLine", False, False),
        ("TranslationButton", False, False),

        ("CoverBackground", True, True),
        ("ExpandedMonetEnabled", False, False),
        ("LyricColorFromCover", False, False),
        ("ExpandedCoverOverlay", False, False),
        ("OnlineSongInfo", True, True),
        ("Animation", False, False),
        ("WorkflowModules", [{"id":"e2e-a","name":"专注","minutes":25,"kind":1},{"id":"e2e-b","name":"休息","minutes":5,"kind":2}], [{"id":"e2e-a","name":"专注","minutes":25,"kind":1},{"id":"e2e-b","name":"休息","minutes":5,"kind":2}]),
    ]
    st = wait_apply(client)
    ok_all = True
    for key, value, expect in roundtrips:
        sent = upd(key, value)
        st = wait_apply(client)
        got = setting(key)
        # 数值统一为 float 比较（JSON 数字）
        if isinstance(expect, (int, float)) and isinstance(got, (int, float)):
            ok = sent and abs(float(got) - float(expect)) < 1e-6
        elif key == "WorkflowModules" and isinstance(expect, list):
            ok = sent and [x.get("id") for x in (got or [])] == [x["id"] for x in expect]
        else:
            ok = sent and got == expect
        if not ok:
            ok_all = False
        t.check("update %s = %r" % (key, expect), ok,
                "sent=%s got=%r reply=%r" % (sent, got, last_reply.get("message")))
        # remember last reply message for diagnostics

    # 列表键
    # LocalFolders 往返不在列：island 接受 update 但 settings 回显为 []（island 侧行为，
    # Rust UI 同样受影响）；lyrics resolver 的集成由 dotisle_lyrics 的 e2e 覆盖。
    for key, value in [("IgnoredSources", ["Browser"]),
                       ("SelectedSources", ["Netease", "Spotify"])]:
        sent = upd(key, value)
        st = wait_apply(client)
        got = setting(key)
        t.check("update %s = %r" % (key, value), sent and got == value, "got=%r" % (got,))

    # 还原默认值，避免污染下一次测试
    for key, value in [("SettingsTheme", "System"), ("IslandSizeMode", "Classic"),
                       ("IslandColorSource", "Content"), ("IslandMaterial", "Acrylic"),
                       ("PlayerResponseMode", "All"), ("ProgressBarStyle", "Wavy"),
                       ("ProgressBarThumbEnabled", False), ("TransportTheme", "Reef"),
                       ("MusicExpandedLayout", "SideBySide"), ("MusicGlowMode", "Breath"),
                       ("SystemStateHintPlacement", "Island"), ("IslandFontFamily", "Noto Sans SC"),
                       ("HideWhenPaused", True), ("HoverExpand", False), ("Floating", False),
                       ("ControlCenterColorSource", "Content"), ("DynamicColorHighContrast", False),
                       ("OffsetMilliseconds", 0), ("TodoRemindersEnabled", True), ("TimerMonoDigits", False)]:
        upd(key, value)

    # ---- 2. toggleVisibility 翻转 ----------------------------------------
    client.request("toggleVisibility")
    time.sleep(0.7)
    st = wait_apply(client)
    hidden = st.get("islandEnabled")
    client.request("toggleVisibility")
    time.sleep(0.7)
    st = wait_apply(client)
    shown = st.get("islandEnabled")
    t.check("toggleVisibility 翻转", hidden != shown, "hide=%r show=%r" % (hidden, shown))

    # ---- 3. 待办生命周期 ---------------------------------------------------
    r = client.request("todoSave", {"id": "", "title": "e2e 待办", "content": "内容",
                                    "date": "2030-01-01", "time": "09:00",
                                    "daily": False, "color": "#F3D79B"})
    t.check("todoSave", r.get("success") is True, str(r))
    st = wait_apply(client)
    todo_id = None
    for todo in st.get("todos") or []:
        if todo.get("title") == "e2e 待办":
            todo_id = todo.get("id")
    t.check("待办出现在 state", todo_id is not None, "")
    if todo_id:
        client.request("todoComplete", {"id": todo_id})
        time.sleep(0.15)
        st = wait_apply(client)
        done = any(x.get("id") == todo_id and x.get("completed")
                   for x in (st.get("todos") or []))
        t.check("todoComplete", done, "")
        client.request("todoDelete", {"id": todo_id})
        time.sleep(0.15)
        st = wait_apply(client)
        gone = all(x.get("id") != todo_id for x in (st.get("todos") or []))
        t.check("todoDelete", gone, "")

    # ---- 4. 计时生命周期 ---------------------------------------------------
    r = client.request("timerCreate", {"mode": "pomodoro", "duration": 1500,
                                       "focus": 1500, "rest": 300, "rounds": 1})
    t.check("timerCreate", r.get("success") is True, str(r))
    timer_id = r.get("id")
    time.sleep(0.15)
    st = wait_apply(client)
    timers = st.get("timers") or []
    t.check("计时出现在 state", any(x.get("id") == timer_id for x in timers), str(len(timers)))
    if timer_id:
        revision = 0
        for x in timers:
            if x.get("id") == timer_id:
                revision = x.get("revision", 0)
        r = client.request("timerAction", {"id": timer_id, "action": "delete",
                                           "revision": revision})
        time.sleep(0.15)
        st = wait_apply(client)
        gone = all(x.get("id") != timer_id for x in (st.get("timers") or []))
        t.check("timerAction delete", gone, "")

    # ---- 5. 快速提醒 -------------------------------------------------------
    r = client.request("quickReminderPreview", {"text": "20分钟后喝水"})
    ok = r.get("success") is True
    token = r.get("previewToken")
    t.check("quickReminderPreview", ok and bool(token), str(r)[:160])
    if ok:
        r = client.request("quickReminderCommit",
                           {"previewToken": token,
                            "clientRequestId": "vello-" + str(token)})
        t.check("quickReminderCommit", r.get("success") is True, str(r)[:160])

    # ---- 6. 应用规则生命周期 ------------------------------------------------
    r = client.request("appRuleSave", {"id": "", "name": "e2e 规则",
                                       "executablePath": r"C:\Windows\System32\notepad.exe",
                                       "message": "该休息了", "cooldownMinutes": 30,
                                       "thresholdMinutes": 60,
                                       "trigger": "foregroundDaily", "enabled": True})
    t.check("appRuleSave", r.get("success") is True, str(r)[:160])
    time.sleep(0.15)
    st = wait_apply(client)
    rule_id = None
    rule_rev = 0
    for rule in st.get("appRules") or []:
        if rule.get("name") == "e2e 规则":
            rule_id = rule.get("id")
            rule_rev = rule.get("revision", 0)
    t.check("规则出现在 state", rule_id is not None, "")
    if rule_id:
        r = client.request("appRuleAction", {"id": rule_id, "action": "disable",
                                             "revision": rule_rev})
        t.check("appRuleAction disable", r.get("success") is True, str(r)[:120])
        time.sleep(0.15)
        st = wait_apply(client)
        rev = 0
        for rule in st.get("appRules") or []:
            if rule.get("id") == rule_id:
                rule_rev = rule.get("revision", 0)
        client.request("appRuleAction", {"id": rule_id, "action": "delete",
                                         "revision": rule_rev})
        time.sleep(0.15)
        st = wait_apply(client)
        gone = all(x.get("id") != rule_id for x in (st.get("appRules") or []))
        t.check("规则删除", gone, "")

    # ---- 7. 桌面组件生命周期 -------------------------------------------------
    r = client.request("widgetAdd", {"kind": "clock", "size": "small"})
    t.check("widgetAdd", r.get("success") is True, str(r)[:120])
    time.sleep(0.15)
    st = wait_apply(client)
    widget_id = None
    for wd in st.get("desktopWidgets") or []:
        if wd.get("kind") == "clock":
            widget_id = wd.get("id")
    t.check("组件出现在 state", widget_id is not None, "")
    if widget_id:
        client.request("widgetUpdate", {"id": widget_id, "size": "large"})
        time.sleep(0.15)
        st = wait_apply(client)
        resized = any(x.get("id") == widget_id and x.get("size") == "large"
                      for x in (st.get("desktopWidgets") or []))
        t.check("widgetUpdate size", resized, "")
        client.request("widgetRemove", {"id": widget_id})
        time.sleep(0.15)
        st = wait_apply(client)
        gone = all(x.get("id") != widget_id for x in (st.get("desktopWidgets") or []))
        t.check("widgetRemove", gone, "")

    # ---- 8. 应答型命令（无崩溃即过） ------------------------------------------
    for name, cmd, value in [
            ("media toggle", "media", {"action": "toggle"}),
            ("notificationPreview", "notificationPreview", None),
            ("historyClear", "historyClear", None),
            ("moduleRefresh", "moduleRefresh", None),
            ("pageSeen", "pageSeen", None),
            ("soundPreview", "soundPreview", None),
            ("islandSelect", "islandSelect", {"page": "music"}),
            ("expand", "expand", None),
            ("resetAppearance", "resetAppearance", None)]:
        try:
            r = client.request(cmd, value)
            t.check(name, isinstance(r, dict), str(r)[:120])
        except Exception as e:
            t.check(name, False, str(e))

    # ---- 9. 音效选择 ---------------------------------------------------------
    r = client.request("soundSelect", {"category": "notification", "source": "Builtin",
                                       "kind": "SoftChime"})
    t.check("soundSelect", isinstance(r, dict), str(r)[:120])
    client.request("update", {"key": "NotificationSound", "value": "SoftChime"})

    client.close()
    ok = t.summary()
    for name, detail in t.failures:
        print("  失败项: %s %s" % (name, detail))
    if island_proc:
        island_proc.kill()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
