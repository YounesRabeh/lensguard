/* SPDX-License-Identifier: GPL-3.0-or-later */
typedef unsigned int u32;
typedef signed int s32;
typedef unsigned long long u64;
typedef signed long long s64;

#define SEC(name) __attribute__((section(name), used))
#define BPF_MAP_TYPE_HASH 1
#define BPF_MAP_TYPE_PERF_EVENT_ARRAY 4
#define BPF_ANY 0
#define BPF_F_CURRENT_CPU 0xffffffffULL
#define VIDIOC_STREAMON 0x40045612U
#define VIDIOC_STREAMOFF 0x40045613U

struct bpf_map_def {
    u32 type;
    u32 key_size;
    u32 value_size;
    u32 max_entries;
    u32 map_flags;
};

struct syscall_enter {
    u64 common;
    s64 syscall_nr;
    u64 args[6];
};

struct syscall_exit {
    u64 common;
    s64 syscall_nr;
    s64 ret;
};

struct pending_ioctl {
    s32 fd;
    u32 command;
};

struct capture_key {
    u32 tgid;
    s32 fd;
};

struct kernel_event {
    u64 timestamp_ns;
    u64 pid_tgid;
    u64 uid_gid;
    s32 fd;
    u32 operation;
    s64 result;
};

struct bpf_map_def SEC("maps") EVENTS = {
    .type = BPF_MAP_TYPE_PERF_EVENT_ARRAY,
    .key_size = sizeof(u32),
    .value_size = sizeof(u32),
};

struct bpf_map_def SEC("maps") PENDING_IOCTL = {
    .type = BPF_MAP_TYPE_HASH,
    .key_size = sizeof(u64),
    .value_size = sizeof(struct pending_ioctl),
    .max_entries = 4096,
};

struct bpf_map_def SEC("maps") PENDING_CLOSE = {
    .type = BPF_MAP_TYPE_HASH,
    .key_size = sizeof(u64),
    .value_size = sizeof(s32),
    .max_entries = 4096,
};

struct bpf_map_def SEC("maps") ACTIVE_CAPTURE = {
    .type = BPF_MAP_TYPE_HASH,
    .key_size = sizeof(struct capture_key),
    .value_size = sizeof(u32),
    .max_entries = 16384,
};

struct bpf_map_def SEC("maps") ACTIVE_TGID = {
    .type = BPF_MAP_TYPE_HASH,
    .key_size = sizeof(u32),
    .value_size = sizeof(u32),
    .max_entries = 4096,
};

static u64 (*bpf_get_current_pid_tgid)(void) = (void *)14;
static u64 (*bpf_get_current_uid_gid)(void) = (void *)15;
static u64 (*bpf_ktime_get_ns)(void) = (void *)5;
static void *(*bpf_map_lookup_elem)(void *map, const void *key) = (void *)1;
static long (*bpf_map_update_elem)(void *map, const void *key, const void *value, u64 flags) = (void *)2;
static long (*bpf_map_delete_elem)(void *map, const void *key) = (void *)3;
static long (*bpf_perf_event_output)(void *ctx, void *map, u64 flags, const void *data, u64 size) = (void *)25;

static __attribute__((always_inline)) void emit(void *ctx, s32 fd, u32 operation, s64 result) {
    struct kernel_event event = {
        .timestamp_ns = bpf_ktime_get_ns(),
        .pid_tgid = bpf_get_current_pid_tgid(),
        .uid_gid = bpf_get_current_uid_gid(),
        .fd = fd,
        .operation = operation,
        .result = result,
    };
    bpf_perf_event_output(ctx, &EVENTS, BPF_F_CURRENT_CPU, &event, sizeof(event));
}

static __attribute__((always_inline)) void increment_tgid(u32 tgid) {
    u32 one = 1;
    u32 *count = bpf_map_lookup_elem(&ACTIVE_TGID, &tgid);
    if (count)
        __sync_fetch_and_add(count, 1);
    else
        bpf_map_update_elem(&ACTIVE_TGID, &tgid, &one, BPF_ANY);
}

static __attribute__((always_inline)) void decrement_tgid(u32 tgid) {
    u32 *count = bpf_map_lookup_elem(&ACTIVE_TGID, &tgid);
    if (!count)
        return;
    if (*count <= 1)
        bpf_map_delete_elem(&ACTIVE_TGID, &tgid);
    else
        /* Clang 18's BPF backend cannot select AtomicLoadSub. BPF XADD with
         * the two's-complement value is the equivalent atomic decrement. */
        __sync_fetch_and_add(count, (u32)-1);
}

SEC("tracepoint/syscalls/sys_enter_ioctl")
int enter_ioctl(struct syscall_enter *ctx) {
    u32 command = (u32)ctx->args[1];
    if (command != VIDIOC_STREAMON && command != VIDIOC_STREAMOFF)
        return 0;
    u64 pid_tgid = bpf_get_current_pid_tgid();
    struct pending_ioctl pending = {.fd = (s32)ctx->args[0], .command = command};
    bpf_map_update_elem(&PENDING_IOCTL, &pid_tgid, &pending, BPF_ANY);
    return 0;
}

SEC("tracepoint/syscalls/sys_exit_ioctl")
int exit_ioctl(struct syscall_exit *ctx) {
    u64 pid_tgid = bpf_get_current_pid_tgid();
    struct pending_ioctl *pending = bpf_map_lookup_elem(&PENDING_IOCTL, &pid_tgid);
    if (!pending)
        return 0;
    struct pending_ioctl copy = *pending;
    bpf_map_delete_elem(&PENDING_IOCTL, &pid_tgid);
    if (ctx->ret != 0)
        return 0;
    u32 tgid = (u32)(pid_tgid >> 32);
    struct capture_key key = {.tgid = tgid, .fd = copy.fd};
    if (copy.command == VIDIOC_STREAMON) {
        u32 one = 1;
        if (!bpf_map_lookup_elem(&ACTIVE_CAPTURE, &key)) {
            bpf_map_update_elem(&ACTIVE_CAPTURE, &key, &one, BPF_ANY);
            increment_tgid(tgid);
        }
        emit(ctx, copy.fd, 1, ctx->ret);
    } else {
        if (bpf_map_lookup_elem(&ACTIVE_CAPTURE, &key)) {
            bpf_map_delete_elem(&ACTIVE_CAPTURE, &key);
            decrement_tgid(tgid);
        }
        emit(ctx, copy.fd, 2, ctx->ret);
    }
    return 0;
}

SEC("tracepoint/syscalls/sys_enter_close")
int enter_close(struct syscall_enter *ctx) {
    u64 pid_tgid = bpf_get_current_pid_tgid();
    struct capture_key key = {.tgid = (u32)(pid_tgid >> 32), .fd = (s32)ctx->args[0]};
    if (bpf_map_lookup_elem(&ACTIVE_CAPTURE, &key))
        bpf_map_update_elem(&PENDING_CLOSE, &pid_tgid, &key.fd, BPF_ANY);
    return 0;
}

SEC("tracepoint/syscalls/sys_exit_close")
int exit_close(struct syscall_exit *ctx) {
    u64 pid_tgid = bpf_get_current_pid_tgid();
    s32 *fd = bpf_map_lookup_elem(&PENDING_CLOSE, &pid_tgid);
    if (!fd)
        return 0;
    s32 copy = *fd;
    bpf_map_delete_elem(&PENDING_CLOSE, &pid_tgid);
    if (ctx->ret != 0)
        return 0;
    u32 tgid = (u32)(pid_tgid >> 32);
    struct capture_key key = {.tgid = tgid, .fd = copy};
    bpf_map_delete_elem(&ACTIVE_CAPTURE, &key);
    decrement_tgid(tgid);
    emit(ctx, copy, 3, ctx->ret);
    return 0;
}

SEC("tracepoint/sched/sched_process_exit")
int process_exit(void *ctx) {
    u64 pid_tgid = bpf_get_current_pid_tgid();
    u32 pid = (u32)pid_tgid;
    u32 tgid = (u32)(pid_tgid >> 32);
    if (pid == tgid && bpf_map_lookup_elem(&ACTIVE_TGID, &tgid)) {
        bpf_map_delete_elem(&ACTIVE_TGID, &tgid);
        emit(ctx, -1, 4, 0);
    }
    return 0;
}

char LICENSE[] SEC("license") = "GPL";
