# Use only with an own child: gdb -q -batch -x THIS_FILE --args CLIENT ARGS...
# Stop-the-world stack samples are not exclusive CPU timings or an FPS benchmark.
set pagination off
set confirm off
set width 0
set print thread-events off
set print frame-arguments none
set print entry-values no
set disable-randomization off
set startup-with-shell off
set follow-fork-mode parent
set detach-on-fork on
handle SIGINT stop print nopass

python
import contextlib
import os
import signal
import threading
import time
import gdb

START_DELAY = 30.0
SAMPLE_INTERVAL = 0.4
MAX_SAMPLES = 16
MAX_THREADS = 128
MAX_FRAMES = 12
MAX_NAME = 300
MAX_RUNTIME = 180.0

def short(value):
    return " ".join(str(value).split())[:MAX_NAME]

class StackSampler:
    def __init__(self):
        self.cancel = threading.Event()
        self.started = threading.Event()
        self.requested = threading.Event()
        self.signal_sent = threading.Event()
        self.completed = threading.Event()
        self.capturing = threading.Event()
        self.pid = 0
        self.begin = 0.0
        self.samples = 0
        self.exited = False
        self.failed = False
        self.previous_ticks = {}
        gdb.events.cont.connect(self.on_continue)
        gdb.events.stop.connect(self.on_stop)
        gdb.events.exited.connect(self.on_exit)

    def on_continue(self, event):
        if not self.started.is_set():
            self.pid = gdb.selected_inferior().pid
            self.begin = time.monotonic()
            self.started.set()

    def on_exit(self, event):
        self.exited = True
        if getattr(event, "exit_code", 0) != 0:
            self.failed = True
        self.cancel.set()
        self.completed.set()
        gdb.write("[stack-sampler] Child exited (code=%s); collected %d/%d samples.\n"
                  % (getattr(event, "exit_code", "signal"), self.samples, MAX_SAMPLES))

    def ticks(self, tid):
        try:
            with open("/proc/%d/task/%d/stat" % (self.pid, tid)) as source:
                line = source.read(4096)
            # Fields after the parenthesized comm start at field 3 (state).
            fields = line[line.rfind(")") + 2:].split()
            return int(fields[11]) + int(fields[12])
        except (OSError, ValueError, IndexError):
            return None

    def dump(self):
        inferior = gdb.selected_inferior()
        if inferior.pid != self.pid:
            raise RuntimeError("Refusing to sample another inferior")
        threads = list(inferior.threads())
        gdb.write("\n[stack-sample %02d/%02d] elapsed=%.3fs threads=%d; "
                  "CPU ticks are per-thread deltas, not exclusive body time\n"
                  % (self.samples + 1, MAX_SAMPLES,
                     time.monotonic() - self.begin, len(threads)))
        saved = gdb.selected_thread()
        rows = []
        for thread in threads:
            tid = thread.ptid[1] or thread.ptid[0]
            ticks = self.ticks(tid)
            previous = self.previous_ticks.get(tid)
            delta = None if ticks is None or previous is None else max(0, ticks - previous)
            if ticks is not None:
                self.previous_ticks[tid] = ticks
            rows.append((delta, thread.num, tid, thread))
        rows.sort(key=lambda row: (-(row[0] if row[0] is not None else -1), row[1]))
        try:
            for delta, number, tid, thread in rows[:MAX_THREADS]:
                gdb.write("TID=%d gdb-thread=%d cpu_tick_delta=%s name=%s\n"
                          % (tid, number, "n/a" if delta is None else delta,
                             short(thread.name or "unnamed")))
                try:
                    thread.switch()
                    frame = gdb.newest_frame()
                    for depth in range(MAX_FRAMES):
                        if frame is None:
                            break
                        gdb.write("  #%02d 0x%x %s\n"
                                  % (depth, frame.pc(), short(frame.name() or "??")))
                        frame = frame.older()
                except gdb.error as error:
                    gdb.write("  [unwind unavailable] %s\n" % short(error))
            if len(rows) > MAX_THREADS:
                gdb.write("[stack-sampler] Thread output capped at %d.\n" % MAX_THREADS)
        finally:
            if saved is not None and saved.is_valid():
                saved.switch()

    def on_stop(self, event):
        if (self.requested.is_set() and isinstance(event, gdb.SignalEvent)
                and event.stop_signal == "SIGINT" and self.samples < MAX_SAMPLES):
            self.capturing.set()
            try:
                self.dump()
                self.samples += 1
            except Exception as error:
                self.failed = True
                gdb.write("[stack-sampler] Capture failed: %s\n" % short(error))
            finally:
                self.requested.clear()
                self.capturing.clear()
                self.completed.set()
            return
        self.failed = True
        self.cancel.set()
        self.completed.set()
        gdb.write("[stack-sampler] Unexpected stop (%s); not auto-resuming.\n"
                  % short(getattr(event, "stop_signal", type(event).__name__)))

    def signal_child(self):
        # Called on GDB's main thread via post_event. SIGINT is intercepted by
        # GDB (stop/nopass); the application never receives our sampling signal.
        if (not self.cancel.is_set() and self.requested.is_set()
                and not self.signal_sent.is_set() and self.pid):
            self.signal_sent.set()
            try:
                os.kill(self.pid, signal.SIGINT)
            except ProcessLookupError:
                pass

    def abort(self):
        self.failed = True
        self.cancel.set()
        gdb.write("[stack-sampler] Bounded timeout; stopping own child.\n")
        if self.pid and not self.exited:
            try:
                os.kill(self.pid, signal.SIGINT)
            except ProcessLookupError:
                pass

    def timer(self):
        if not self.started.wait(10.0):
            return
        if self.cancel.wait(START_DELAY):
            return
        for index in range(MAX_SAMPLES):
            if self.cancel.is_set():
                return
            self.completed.clear()
            self.signal_sent.clear()
            self.requested.set()
            gdb.post_event(self.signal_child)
            # A synchronous native target may not process post_event until it
            # stops. Only then use a direct SIGINT fallback, without GDB APIs.
            if not self.completed.wait(5.0) and not self.capturing.is_set():
                self.signal_child()
            if not self.completed.wait(15.0):
                gdb.post_event(self.abort)
                try:
                    os.kill(self.pid, signal.SIGINT)
                except ProcessLookupError:
                    pass
                return
            if index + 1 < MAX_SAMPLES and self.cancel.wait(SAMPLE_INTERVAL):
                return
        # Sampling ends after sixteen stops; let the benchmark child exit
        # naturally, with a final bound if the fixture itself never finishes.
        remaining = max(0.0, MAX_RUNTIME - (time.monotonic() - self.begin))
        if not self.cancel.wait(remaining):
            gdb.post_event(self.abort)
            try:
                os.kill(self.pid, signal.SIGINT)
            except ProcessLookupError:
                pass

sampler = StackSampler()
with (gdb.blocked_signals() if hasattr(gdb, "blocked_signals") else contextlib.nullcontext()):
    worker = threading.Thread(target=sampler.timer, name="bounded-stack-sampler", daemon=True)
    worker.start()
gdb.write("[stack-sampler] Own-child mode; first sample after 30s, at most 16 stops.\n")
try:
    gdb.execute("run")
    while not sampler.exited and not sampler.failed:
        gdb.execute("continue")
except (gdb.error, KeyboardInterrupt) as error:
    sampler.failed = True
    gdb.write("[stack-sampler] Debugger command ended: %s\n" % short(error))
finally:
    sampler.cancel.set()
    worker.join(timeout=0.2)
gdb.execute("quit %d" % int(sampler.failed))
end
