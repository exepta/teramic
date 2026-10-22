// Per-thread userspace CPU-clock samples. No application instrumentation required.
// Build: cc -O2 -Wall -Wextra -Werror tools/benchmarks/sample_cpu.c -o target/sample-cpu
// Probe: target/sample-cpu --probe [PID]
// Record: target/sample-cpu PID SECONDS OUTPUT_PREFIX
#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <linux/perf_event.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

#define MAX_THREADS 1024
#define RING_PAGES 8
#define SAMPLE_NANOS UINT64_C(5000000)

struct thread_event {
    pid_t tid;
    int fd;
    struct perf_event_mmap_page *page;
    size_t map_length;
    uint64_t tail;
};

static struct thread_event events[MAX_THREADS];
static size_t event_count;
static uint64_t samples;
static uint64_t lost;
static uint64_t chained;
static volatile sig_atomic_t stopping;

static void stop_recording(int signal_number) {
    (void)signal_number;
    stopping = 1;
}

static double monotonic_seconds(void) {
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (double)now.tv_sec + (double)now.tv_nsec / 1e9;
}

static int open_event(pid_t tid) {
    struct perf_event_attr attr = {
        .type = PERF_TYPE_SOFTWARE,
        .size = sizeof(struct perf_event_attr),
        .config = PERF_COUNT_SW_CPU_CLOCK,
        .sample_period = SAMPLE_NANOS,
        .sample_type = PERF_SAMPLE_IP | PERF_SAMPLE_TID | PERF_SAMPLE_CALLCHAIN,
        .disabled = 1,
        .exclude_kernel = 1,
        .exclude_hv = 1,
        .exclude_guest = 1,
        .exclude_callchain_kernel = 1,
        .wakeup_events = 1,
        .sample_max_stack = 64,
    };
    return (int)syscall(SYS_perf_event_open, &attr, tid, -1, -1, PERF_FLAG_FD_CLOEXEC);
}

static void copy_ring(const struct thread_event *event, uint64_t offset, void *target, size_t length) {
    const char *data = (const char *)event->page + event->page->data_offset;
    const size_t size = (size_t)event->page->data_size;
    const size_t position = (size_t)(offset % size);
    const size_t first = length < size - position ? length : size - position;
    memcpy(target, data + position, first);
    if (first < length) {
        memcpy((char *)target + first, data, length - first);
    }
}

static uint64_t read_u64(const unsigned char *record, size_t offset) {
    uint64_t value;
    memcpy(&value, record + offset, sizeof(value));
    return value;
}

static void drain_event(struct thread_event *event, FILE *output) {
    if (!event->page) {
        return;
    }
    uint64_t head = __atomic_load_n(&event->page->data_head, __ATOMIC_ACQUIRE);
    while (event->tail < head) {
        struct perf_event_header header;
        copy_ring(event, event->tail, &header, sizeof(header));
        if (header.size < sizeof(header) || header.size > 65528) {
            fprintf(stderr, "Malformed perf record for tid %d\n", event->tid);
            stopping = 1;
            break;
        }
        unsigned char record[65536];
        copy_ring(event, event->tail, record, header.size);
        if (header.type == PERF_RECORD_SAMPLE && header.size >= 32) {
            uint64_t ip = read_u64(record, 8);
            uint64_t nr = read_u64(record, 24);
            if (nr <= (header.size - 32U) / 8U) {
                fprintf(output, "%d\t0x%" PRIx64, event->tid, ip);
                size_t depth = 0;
                for (uint64_t i = 0; i < nr; ++i) {
                    uint64_t address = read_u64(record, 32 + i * 8);
                    if ((int64_t)address >= 0 && address != ip) {
                        fprintf(output, "\t0x%" PRIx64, address);
                        ++depth;
                    }
                }
                fputc('\n', output);
                ++samples;
                chained += depth > 0;
            }
        } else if (header.type == PERF_RECORD_LOST && header.size >= 24) {
            lost += read_u64(record, 16);
        }
        event->tail += header.size;
    }
    __atomic_store_n(&event->page->data_tail, event->tail, __ATOMIC_RELEASE);
}

static int discover_threads(pid_t pid, FILE *names) {
    char path[128];
    snprintf(path, sizeof(path), "/proc/%d/task", pid);
    DIR *directory = opendir(path);
    if (!directory) {
        return -1;
    }
    struct dirent *entry;
    while ((entry = readdir(directory))) {
        char *end;
        long value = strtol(entry->d_name, &end, 10);
        if (*end || value <= 0) {
            continue;
        }
        pid_t tid = (pid_t)value;
        bool known = false;
        for (size_t i = 0; i < event_count; ++i) {
            known |= events[i].tid == tid;
        }
        if (known || event_count == MAX_THREADS) {
            continue;
        }
        int fd = open_event(tid);
        if (fd < 0) {
            if (errno != ESRCH) {
                fprintf(stderr, "perf_event_open tid=%d: %s\n", tid, strerror(errno));
            }
            continue;
        }
        size_t length = (RING_PAGES + 1) * (size_t)sysconf(_SC_PAGESIZE);
        void *mapping = mmap(NULL, length, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
        if (mapping == MAP_FAILED) {
            fprintf(stderr, "mmap tid=%d: %s\n", tid, strerror(errno));
            close(fd);
            continue;
        }
        events[event_count++] = (struct thread_event){
            .tid = tid, .fd = fd, .page = mapping, .map_length = length, .tail = 0,
        };
        snprintf(path, sizeof(path), "/proc/%d/task/%d/comm", pid, tid);
        FILE *comm = fopen(path, "r");
        char name[256] = "unknown";
        if (comm) {
            if (fgets(name, sizeof(name), comm)) {
                name[strcspn(name, "\n\t")] = 0;
            }
            fclose(comm);
        }
        fprintf(names, "%d\t%s\n", tid, name);
        ioctl(fd, PERF_EVENT_IOC_ENABLE, 0);
    }
    closedir(directory);
    return 0;
}

static FILE *open_output(const char *prefix, const char *suffix) {
    char *path;
    if (asprintf(&path, "%s.%s", prefix, suffix) < 0) {
        return NULL;
    }
    FILE *output = fopen(path, "wx");
    if (!output) {
        fprintf(stderr, "%s: %s\n", path, strerror(errno));
    }
    free(path);
    return output;
}

int main(int argc, char **argv) {
    if (argc >= 2 && strcmp(argv[1], "--probe") == 0) {
        pid_t pid = argc >= 3 ? (pid_t)strtol(argv[2], NULL, 10) : getpid();
        int fd = open_event(pid);
        if (fd < 0) {
            fprintf(stderr, "CPU-clock perf_event_open pid=%d: %s\n", pid, strerror(errno));
            return 1;
        }
        close(fd);
        puts("CPU-clock userspace sampling permitted");
        return 0;
    }
    if (argc != 4) {
        fprintf(stderr, "Usage: %s PID SECONDS OUTPUT_PREFIX\n", argv[0]);
        return 2;
    }
    pid_t pid = (pid_t)strtol(argv[1], NULL, 10);
    double duration = strtod(argv[2], NULL);
    if (pid <= 0 || !(duration > 0 && duration <= 3600)) {
        fputs("PID must be positive and seconds must be in (0, 3600]\n", stderr);
        return 2;
    }
    FILE *output = open_output(argv[3], "samples.tsv");
    FILE *names = open_output(argv[3], "threads.tsv");
    FILE *maps = open_output(argv[3], "maps");
    if (!output || !names || !maps) {
        return 1;
    }
    char path[128];
    snprintf(path, sizeof(path), "/proc/%d/maps", pid);
    FILE *source = fopen(path, "r");
    if (!source) {
        perror(path);
        return 1;
    }
    char line[8192];
    while (fgets(line, sizeof(line), source)) {
        fputs(line, maps);
    }
    fclose(source);
    fclose(maps);
    signal(SIGINT, stop_recording);
    signal(SIGTERM, stop_recording);
    if (discover_threads(pid, names) < 0 || !event_count) {
        fputs("No thread sampling events could be opened\n", stderr);
        return 1;
    }
    fprintf(output, "# pid=%d sample_cpu_ns=%" PRIu64 "\n", pid, SAMPLE_NANOS);
    double start = monotonic_seconds();
    double next_scan = start + 1.0;
    while (!stopping && monotonic_seconds() - start < duration) {
        struct pollfd polling[MAX_THREADS];
        for (size_t i = 0; i < event_count; ++i) {
            polling[i] = (struct pollfd){.fd = events[i].fd, .events = POLLIN};
        }
        poll(polling, (nfds_t)event_count, 20);
        for (size_t i = 0; i < event_count; ++i) {
            drain_event(&events[i], output);
            if (events[i].fd >= 0 && (polling[i].revents & POLLHUP)) {
                munmap(events[i].page, events[i].map_length);
                close(events[i].fd);
                events[i].page = NULL;
                events[i].fd = -1;
            }
        }
        if (monotonic_seconds() >= next_scan) {
            if (discover_threads(pid, names) < 0) {
                break;
            }
            next_scan += 1.0;
        }
    }
    for (size_t i = 0; i < event_count; ++i) {
        if (events[i].fd < 0) {
            continue;
        }
        ioctl(events[i].fd, PERF_EVENT_IOC_DISABLE, 0);
        drain_event(&events[i], output);
        munmap(events[i].page, events[i].map_length);
        close(events[i].fd);
    }
    fprintf(stderr, "threads=%zu samples=%" PRIu64 " lost=%" PRIu64
            " samples_with_caller=%" PRIu64 " wall_seconds=%.3f\n",
            event_count, samples, lost, chained, monotonic_seconds() - start);
    fclose(output);
    fclose(names);
    return samples ? 0 : 1;
}
