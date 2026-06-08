# Nsight Systems SQLite 数据库表结构说明

> 数据来源: `sglang_profile.sqlite` (NVIDIA Nsight Systems 2025.6.3 导出)
> 分析目标: sglang 推理服务 (Qwen3-8B, RTX 3090)

## 1. CUPTI Activity 表 (GPU 活动记录)

### CUPTI_ACTIVITY_KIND_KERNEL
CUDA Kernel 执行记录，是最核心的性能分析表。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | Kernel 开始时间 (ns) |
| end | INTEGER | Kernel 结束时间 (ns) |
| deviceId | INTEGER | GPU 设备 ID |
| contextId | INTEGER | CUDA Context ID |
| greenContextId | INTEGER | Green Context ID (可选) |
| streamId | INTEGER | CUDA Stream ID |
| correlationId | INTEGER | 关联 ID，与 Runtime API 调用关联 |
| globalPid | INTEGER | 全局进程 ID |
| demangledName | INTEGER | Kernel 解码后名称 (StringIds 外键) |
| shortName | INTEGER | Kernel 简称 (StringIds 外键) |
| mangledName | INTEGER | Kernel 原始名称 (StringIds 外键) |
| launchType | INTEGER | 启动类型 (→ ENUM_CUDA_KERNEL_LAUNCH_TYPE) |
| cacheConfig | INTEGER | 缓存配置 (→ ENUM_CUDA_FUNC_CACHE_CONFIG) |
| registersPerThread | INTEGER | 每线程寄存器数 |
| gridX/Y/Z | INTEGER | Grid 维度 |
| blockX/Y/Z | INTEGER | Block 维度 |
| staticSharedMemory | INTEGER | 静态共享内存 (bytes) |
| dynamicSharedMemory | INTEGER | 动态共享内存 (bytes) |
| localMemoryPerThread | INTEGER | 每线程本地内存 (bytes) |
| localMemoryTotal | INTEGER | 本地内存总量 (bytes) |
| gridId | INTEGER | Grid ID |
| sharedMemoryExecuted | INTEGER | 实际执行共享内存 |
| graphNodeId | INTEGER | CUDA Graph 节点 ID |
| sharedMemoryLimitConfig | INTEGER | 共享内存限制配置 (→ ENUM_CUDA_SHARED_MEM_LIMIT_CONFIG) |
| graphId | INTEGER | CUDA Graph ID |
| clusterX/Y/Z | INTEGER | Cluster 维度 (线程簇) |
| clusterSchedulingPolicy | INTEGER | Cluster 调度策略 |
| maxPotentialClusterSize | INTEGER | 最大潜在 Cluster 大小 |
| maxActiveClusters | INTEGER | 最大活跃 Cluster 数 |

### CUPTI_ACTIVITY_KIND_MEMCPY
GPU 内存拷贝记录。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | 开始时间 (ns) |
| end | INTEGER | 结束时间 (ns) |
| deviceId | INTEGER | GPU 设备 ID |
| contextId | INTEGER | CUDA Context ID |
| streamId | INTEGER | CUDA Stream ID |
| correlationId | INTEGER | 关联 ID |
| globalPid | INTEGER | 全局进程 ID |
| bytes | INTEGER | 拷贝字节数 |
| copyKind | INTEGER | 拷贝类型 (→ ENUM_CUDA_MEMCPY_OPER) |
| srcKind | INTEGER | 源内存类型 (→ ENUM_CUDA_MEM_KIND) |
| dstKind | INTEGER | 目标内存类型 (→ ENUM_CUDA_MEM_KIND) |
| srcDeviceId | INTEGER | 源设备 ID |
| dstDeviceId | INTEGER | 目标设备 ID |
| migrationCause | INTEGER | 迁移原因 (→ ENUM_CUDA_UNIF_MEM_MIGRATION) |
| virtualAddress | INTEGER | 虚拟地址 |
| copyCount | INTEGER | 拷贝次数 |

### CUPTI_ACTIVITY_KIND_MEMSET
GPU 内存初始化 (memset) 记录。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | 开始时间 (ns) |
| end | INTEGER | 结束时间 (ns) |
| deviceId | INTEGER | GPU 设备 ID |
| contextId | INTEGER | CUDA Context ID |
| streamId | INTEGER | CUDA Stream ID |
| correlationId | INTEGER | 关联 ID |
| globalPid | INTEGER | 全局进程 ID |
| value | INTEGER | 填充值 |
| bytes | INTEGER | 填充字节数 |
| memKind | INTEGER | 内存类型 (→ ENUM_CUDA_MEM_KIND) |

### CUPTI_ACTIVITY_KIND_RUNTIME
CUDA Runtime API 调用记录。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | API 调用开始时间 (ns) |
| end | INTEGER | API 调用结束时间 (ns) |
| eventClass | INTEGER | 事件类 (→ ENUM_NSYS_EVENT_CLASS) |
| globalTid | INTEGER | 全局线程 ID |
| correlationId | INTEGER | 关联 ID，与 GPU Kernel 关联 |
| nameId | INTEGER | API 名称 (StringIds 外键) |
| returnValue | INTEGER | 返回值 |
| callchainId | INTEGER | 调用链 ID (→ OSRT_CALLCHAINS) |

### CUPTI_ACTIVITY_KIND_SYNCHRONIZATION
CUDA 同步事件记录。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | 开始时间 (ns) |
| end | INTEGER | 结束时间 (ns) |
| deviceId | INTEGER | GPU 设备 ID |
| contextId | INTEGER | CUDA Context ID |
| streamId | INTEGER | CUDA Stream ID |
| correlationId | INTEGER | 关联 ID |
| globalPid | INTEGER | 全局进程 ID |
| syncType | INTEGER | 同步类型 (→ ENUM_CUPTI_SYNC_TYPE) |
| eventId | INTEGER | 事件 ID |
| eventSyncId | INTEGER | 事件同步 ID |

### CUPTI_ACTIVITY_KIND_OVERHEAD
CUPTI Profiler 开销记录。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | 开始时间 (ns) |
| end | INTEGER | 结束时间 (ns) |
| eventClass | INTEGER | 事件类 |
| globalTid | INTEGER | 全局线程 ID |
| correlationId | INTEGER | 关联 ID |
| nameId | INTEGER | 名称 (StringIds 外键) |
| overheadType | INTEGER | 开销类型 (→ ENUM_CUPTI_OVERHEAD_TYPE) |

### CUPTI_ACTIVITY_KIND_CUDA_EVENT
CUDA Event 记录 (事件记录/等待)。

| 字段 | 类型 | 说明 |
|------|------|------|
| timestamp | INTEGER | 时间戳 (ns) |
| deviceId | INTEGER | GPU 设备 ID |
| contextId | INTEGER | CUDA Context ID |
| streamId | INTEGER | CUDA Stream ID |
| correlationId | INTEGER | 关联 ID |
| globalPid | INTEGER | 全局进程 ID |
| eventId | INTEGER | Event ID |
| eventSyncId | INTEGER | Event 同步 ID |

### CUPTI_ACTIVITY_KIND_GRAPH_TRACE
CUDA Graph 执行追踪记录。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | 开始时间 (ns) |
| end | INTEGER | 结束时间 (ns) |
| deviceId | INTEGER | GPU 设备 ID |
| contextId | INTEGER | CUDA Context ID |
| streamId | INTEGER | CUDA Stream ID |
| correlationId | INTEGER | 关联 ID |
| globalPid | INTEGER | 全局进程 ID |
| graphId | INTEGER | Graph ID |
| graphExecId | INTEGER | Graph 执行实例 ID |

## 2. CUDA Graph 表

### CUDA_GRAPH_EVENTS
CUDA Graph 相关事件。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | 开始时间 (ns) |
| end | INTEGER | 结束时间 (ns) |
| eventClass | INTEGER | 事件类 |
| globalTid | INTEGER | 全局线程 ID |
| nameId | INTEGER | 名称 (StringIds 外键) |
| graphId | INTEGER | Graph ID |
| originalGraphId | INTEGER | 原始 Graph ID |
| graphExecId | INTEGER | Graph 执行实例 ID |

## 3. NVTX 表 (NVIDIA Tools Extension)

### NVTX_EVENTS
NVTX 标注事件，用于用户自定义性能区间标记。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | 区间开始时间 (ns) |
| end | INTEGER | 区间结束时间 (ns)，Push/Pop 类型可能为空 |
| eventType | INTEGER | 事件类型 (→ ENUM_NSYS_EVENT_TYPE) |
| rangeId | INTEGER | Range ID |
| category | INTEGER | 分类 ID |
| color | INTEGER | 颜色值 |
| text | TEXT | 标注文本 |
| globalTid | INTEGER | 全局线程 ID |
| endGlobalTid | INTEGER | 结束全局线程 ID |
| textId | INTEGER | 文本 ID (StringIds 外键) |
| domainId | INTEGER | NVTX Domain ID |
| uint64Value | INTEGER | 64 位无符号附加值 |
| int64Value | INTEGER | 64 位有符号附加值 |
| doubleValue | REAL | 双精度浮点附加值 |
| uint32Value | INTEGER | 32 位无符号附加值 |
| int32Value | INTEGER | 32 位有符号附加值 |
| floatValue | REAL | 单精度浮点附加值 |
| jsonTextId | INTEGER | JSON 文本 ID (StringIds 外键) |
| jsonText | TEXT | JSON 格式文本 |
| binaryData | TEXT | 二进制数据 |

## 4. OSRT 表 (OS Runtime Trace)

### OSRT_API
操作系统运行时 API 调用记录 (如 pthread, futex, IO 等)。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | API 调用开始时间 (ns) |
| end | INTEGER | API 调用结束时间 (ns) |
| eventClass | INTEGER | 事件类 |
| globalTid | INTEGER | 全局线程 ID |
| nameId | INTEGER | API 名称 (StringIds 外键) |
| returnValue | INTEGER | 返回值 |
| nestingLevel | INTEGER | 嵌套层级 |
| callchainId | INTEGER | 调用链 ID (→ OSRT_CALLCHAINS) |
| argumentsId | INTEGER | 参数 ID |

### OSRT_CALLCHAINS
OSRT API 调用链 (栈回溯)。

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 调用链 ID (主键) |
| symbol | INTEGER | 符号地址 (StringIds 外键) |
| module | INTEGER | 模块地址 (StringIds 外键) |
| kernelMode | INTEGER | 是否内核模式 |
| thumbCode | INTEGER | 是否 Thumb 代码 |
| unresolved | INTEGER | 是否未解析 |
| specialEntry | INTEGER | 特殊入口 |
| originalIP | INTEGER | 原始指令指针 |
| unwindMethod | INTEGER | 展开方法 (→ ENUM_STACK_UNWIND_METHOD) |
| stackDepth | INTEGER | 调用栈深度 |

### OSRT_FILE_ACCESS_DESCRIPTORS
文件访问描述符。

| 字段 | 类型 | 说明 |
|------|------|------|
| fileAccessId | INTEGER | 文件访问 ID |
| processId | INTEGER | 进程 ID |
| openedAt | INTEGER | 打开时间 (ns) |
| closedAt | INTEGER | 关闭时间 (ns) |
| filePath | TEXT | 文件路径 |

### OSRT_FILE_ACCESS_EVENTS
文件访问事件。

| 字段 | 类型 | 说明 |
|------|------|------|
| fileAccessId | INTEGER | 文件访问 ID |
| threadId | INTEGER | 线程 ID |
| startedAt | INTEGER | 事件开始时间 (ns) |
| endedAt | INTEGER | 事件结束时间 (ns) |
| eventType | INTEGER | 事件类型 (→ ENUM_OSRT_FILE_ACCESS_EVENT_TYPE) |
| apiCallId | INTEGER | API 调用 ID |
| bytesProcessed | INTEGER | 处理字节数 |
| context | TEXT | 上下文信息 |

## 5. GPU 内存表

### CUDA_GPU_MEMORY_USAGE_EVENTS
GPU 显存分配/释放事件。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | 事件时间 (ns) |
| globalPid | INTEGER | 全局进程 ID |
| deviceId | INTEGER | GPU 设备 ID |
| contextId | INTEGER | CUDA Context ID |
| address | INTEGER | 内存地址 |
| pc | INTEGER | 程序计数器 |
| bytes | INTEGER | 分配/释放字节数 |
| memKind | INTEGER | 内存类型 (→ ENUM_CUDA_MEM_KIND) |
| memoryOperationType | INTEGER | 操作类型 (→ ENUM_CUDA_DEV_MEM_EVENT_OPER) |
| name | TEXT | 内存操作名称 |
| correlationId | INTEGER | 关联 ID |
| streamId | INTEGER | CUDA Stream ID |
| localMemoryPoolAddress | INTEGER | 本地内存池地址 |
| localMemoryPoolReleaseThreshold | INTEGER | 本地内存池释放阈值 |
| localMemoryPoolSize | INTEGER | 本地内存池大小 |
| localMemoryPoolUtilizedSize | INTEGER | 本地内存池已用大小 |
| importedMemoryPoolAddress | INTEGER | 导入内存池地址 |
| importedMemoryPoolProcessId | INTEGER | 导入内存池进程 ID |

### GPU_CONTEXT_SWITCH_EVENTS
GPU Context 切换事件。

| 字段 | 类型 | 说明 |
|------|------|------|
| tag | INTEGER | 切换标签 (→ ENUM_GPU_CTX_SWITCH) |
| vmId | INTEGER | 虚拟机 ID |
| seqNo | INTEGER | 序列号 |
| contextId | INTEGER | Context ID |
| timestamp | INTEGER | 时间戳 (ns) |
| globalPid | INTEGER | 全局进程 ID |
| gpuId | INTEGER | GPU ID |

## 6. Profiler 相关表

### PROFILER_OVERHEAD
Profiler 自身开销。

| 字段 | 类型 | 说明 |
|------|------|------|
| start | INTEGER | 开销开始时间 (ns) |
| end | INTEGER | 开销结束时间 (ns) |
| globalTid | INTEGER | 全局线程 ID |
| nameId | INTEGER | 名称 (StringIds 外键) |
| overheadType | INTEGER | 开销类型 (→ ENUM_CUPTI_OVERHEAD_TYPE) |

### DIAGNOSTIC_EVENT
诊断事件 (警告、错误等)。

| 字段 | 类型 | 说明 |
|------|------|------|
| timestamp | INTEGER | 时间戳 (ns) |
| timestampType | INTEGER | 时间戳来源 (→ ENUM_DIAGNOSTIC_TIMESTAMP_SOURCE) |
| source | INTEGER | 来源 (→ ENUM_DIAGNOSTIC_SOURCE_TYPE) |
| severity | INTEGER | 严重级别 (→ ENUM_DIAGNOSTIC_SEVERITY_LEVEL) |
| text | TEXT | 诊断文本 |
| globalPid | INTEGER | 全局进程 ID |

## 7. 元数据表

### META_DATA_CAPTURE
采集配置信息 (key-value 形式)。

| 字段 | 类型 | 说明 |
|------|------|------|
| name | TEXT | 配置项名称 |
| value | TEXT | 配置项值 |

关键配置项示例:
- `PROCESS_0:COMMAND` — 被追踪的命令
- `PROCESS_0:ARGUMENT_*` — 命令参数
- `RUN_DURATION_MS` — 采集时长
- `CAPTURE_EVENT_TYPE` — 采集的事件类型

### META_DATA_EXPORT
导出信息 (key-value 形式)。

| 字段 | 类型 | 说明 |
|------|------|------|
| name | TEXT | 导出元数据名称 |
| value | TEXT | 导出元数据值 |

关键信息: 导出工具版本、Schema 版本、平台、时间戳、输入输出路径等。

### ANALYSIS_DETAILS
分析概要。

| 字段 | 类型 | 说明 |
|------|------|------|
| globalVid | INTEGER | 全局 Viewport ID |
| duration | INTEGER | 采集总时长 (ns) |
| startTime | INTEGER | 采集开始时间 (ns) |
| stopTime | INTEGER | 采集结束时间 (ns) |

### ANALYSIS_FILE
分析文件信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 文件 ID |
| filename | TEXT | 文件名 |
| contentId | INTEGER | 内容 ID |
| globalPid | INTEGER | 全局进程 ID |

## 8. 目标信息表

### TARGET_INFO_GPU
GPU 硬件信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| vmId | INTEGER | 虚拟机 ID |
| id | INTEGER | GPU ID |
| name | TEXT | GPU 名称 (如 "NVIDIA GeForce RTX 3090") |
| busLocation | TEXT | PCIe 总线地址 |
| isDiscrete | INTEGER | 是否独立显卡 |
| l2CacheSize | INTEGER | L2 缓存大小 (bytes) |
| totalMemory | INTEGER | 显存总量 (bytes) |
| memoryBandwidth | INTEGER | 显存带宽 (bytes/s) |
| clockRate | INTEGER | 核心频率 (Hz) |
| smCount | INTEGER | SM 数量 |
| uuid | TEXT | GPU UUID |
| chipName | TEXT | 芯片代号 (如 "GA102") |
| computeMajor/Minor | INTEGER | 计算能力主/次版本号 |
| maxThreadsPerBlock | INTEGER | 每块最大线程数 |
| maxBlockDimX/Y/Z | INTEGER | 块最大维度 |
| maxGridDimX/Y/Z | INTEGER | Grid 最大维度 |
| maxRegistersPerBlock | INTEGER | 每块最大寄存器数 |
| maxShmemPerBlock | INTEGER | 每块最大共享内存 |
| threadsPerWarp | INTEGER | 每 Warp 线程数 |
| asyncEngines | INTEGER | 异步引擎数 |

### TARGET_INFO_CUDA_DEVICE
CUDA 设备映射。

| 字段 | 类型 | 说明 |
|------|------|------|
| gpuId | INTEGER | GPU ID |
| cudaId | INTEGER | CUDA 设备 ID |
| pid | INTEGER | 进程 ID |
| uuid | TEXT | UUID |
| numMultiprocessors | INTEGER | SM 数量 |

### TARGET_INFO_CUDA_CONTEXT_INFO
CUDA Context 详细信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| nullStreamId | INTEGER | 默认 Stream ID |
| hwId | INTEGER | 硬件 ID |
| vmId | INTEGER | 虚拟机 ID |
| processId | INTEGER | 进程 ID |
| deviceId | INTEGER | 设备 ID |
| contextId | INTEGER | Context ID |
| parentContextId | INTEGER | 父 Context ID |
| isGreenContext | INTEGER | 是否 Green Context |
| numMultiprocessors | INTEGER | SM 数量 |

### TARGET_INFO_CUDA_STREAM
CUDA Stream 信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| streamId | INTEGER | Stream ID |
| hwId | INTEGER | 硬件 ID |
| processId | INTEGER | 进程 ID |
| contextId | INTEGER | Context ID |
| priority | INTEGER | Stream 优先级 |
| flag | INTEGER | Stream 标志 (→ ENUM_CUPTI_STREAM_TYPE) |

### TARGET_INFO_SESSION_START_TIME
采集会话起始时间。

| 字段 | 类型 | 说明 |
|------|------|------|
| utcEpochNs | INTEGER | UTC 时间戳 (ns) |
| utcTime | TEXT | UTC 时间字符串 |
| localTime | TEXT | 本地时间字符串 |
| systemClockNs | INTEGER | 系统时钟 (ns) |

### TARGET_INFO_SYSTEM_ENV
系统环境信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| globalVid | INTEGER | 全局 Viewport ID |
| devStateName | TEXT | 设备状态名称 |
| name | TEXT | 属性名称 |
| nameEnum | INTEGER | 属性枚举值 |
| value | TEXT | 属性值 |

常见属性: CpuCores, CpuSpeedMhz, CpuArchitecture, CpuDescription 等。

## 9. 进程与线程表

### PROCESSES
被追踪的进程信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| globalPid | INTEGER | 全局进程 ID |
| pid | INTEGER | 系统 PID |
| name | TEXT | 进程名称 |

### ThreadNames
线程名称信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| nameId | INTEGER | 线程名称 (StringIds 外键) |
| priority | INTEGER | 线程优先级 |
| globalTid | INTEGER | 全局线程 ID |

### ProcessStreams
进程与 Stream 关联。

| 字段 | 类型 | 说明 |
|------|------|------|
| globalPid | INTEGER | 全局进程 ID |
| filenameId | INTEGER | 文件名 ID (StringIds 外键) |
| contentId | INTEGER | 内容 ID |

## 10. 字符串表

### StringIds
字符串字典表，其他表通过 id 引用此表获取名称文本。

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 字符串 ID (主键) |
| value | TEXT | 字符串值 |

## 11. 枚举表 (ENUM)

所有 ENUM 表结构统一: `id (主键) | name | label`，用于将数值型字段映射为可读字符串。

### ENUM_CUDA_DEV_MEM_EVENT_OPER
| id | name | label |
|----|------|-------|
| 0 | CUDA_DEV_MEM_EVENT_OPR_ALLOCATION | Allocation |
| 1 | CUDA_DEV_MEM_EVENT_OPR_DEALLOCATION | Deallocation |

### ENUM_CUDA_MEMCPY_OPER
| id | name | label |
|----|------|-------|
| 0 | CUDA_MEMCPY_KIND_UNKNOWN | Unknown |
| 1 | CUDA_MEMCPY_KIND_HTOD | Host-to-Device |
| 2 | CUDA_MEMCPY_KIND_DTOH | Device-to-Host |
| 8 | CUDA_MEMCPY_KIND_DTOD | Device-to-Device |
| 11 | CUDA_MEMCPY_KIND_UVM_HTOD | Unified Host-to-Device |
| 12 | CUDA_MEMCPY_KIND_UVM_DTOH | Unified Device-to-Host |

### ENUM_CUDA_MEM_KIND
| id | name | label |
|----|------|-------|
| 0 | CUDA_MEMOPR_MEMORY_KIND_PAGEABLE | Pageable |
| 1 | CUDA_MEMOPR_MEMORY_KIND_PINNED | Pinned |
| 2 | CUDA_MEMOPR_MEMORY_KIND_DEVICE | Device |
| 4 | CUDA_MEMOPR_MEMORY_KIND_MANAGED | Managed |

### ENUM_CUDA_KERNEL_LAUNCH_TYPE
| id | name | label |
|----|------|-------|
| 0 | CUDA_KERNEL_LAUNCH_TYPE_UNKNOWN | Unknown |
| 1 | CUDA_KERNEL_LAUNCH_TYPE_REGULAR | Regular |
| 2 | CUDA_KERNEL_LAUNCH_TYPE_COOPERATIVE_SINGLE_DEVICE | Coop Single Device |

### ENUM_CUDA_FUNC_CACHE_CONFIG
| id | name | label |
|----|------|-------|
| 0 | CU_FUNC_CACHE_PREFER_NONE | None |
| 1 | CU_FUNC_CACHE_PREFER_SHARED | Shared |
| 2 | CU_FUNC_CACHE_PREFER_L1 | L1 |
| 3 | CU_FUNC_CACHE_PREFER_EQUAL | Equal |

### ENUM_CUDA_SHARED_MEM_LIMIT_CONFIG
| id | name | label |
|----|------|-------|
| 0 | CUDA_SHARED_MEM_LIMIT_DEFAULT | Default |
| 1 | CUDA_SHARED_MEM_LIMIT_OPTIN | Opt in |

### ENUM_CUDA_MEMPOOL_OPER
| id | name | label |
|----|------|-------|
| 1 | ...CREATED | Created |
| 2 | ...DESTROYED | Destroyed |
| 3 | ...TRIMMED | Trimmed |

### ENUM_CUDA_MEMPOOL_TYPE
| id | name | label |
|----|------|-------|
| 1 | ...LOCAL | Local |
| 2 | ...IMPORTED | Imported |

### ENUM_CUDA_UNIF_MEM_ACCESS_TYPE
| id | name | label |
|----|------|-------|
| 1 | ...READ | Read |
| 2 | ...WRITE | Write |
| 3 | ...ATOMIC | Atomic |
| 4 | ...PREFETCH | Prefetch |

### ENUM_CUDA_UNIF_MEM_MIGRATION
| id | name | label |
|----|------|-------|
| 1 | ...USER | User prefetch |
| 2 | ...COHERENCE | Page fault |
| 4 | ...EVICTION | Eviction |

### ENUM_CUPTI_OVERHEAD_TYPE
| id | name | label |
|----|------|-------|
| 2 | OVERHEAD_DRIVER_COMPILER | Driver JIT compiler |
| 3 | OVERHEAD_CUPTI_BUFFER_FLUSH | Buffer flush |
| 4 | OVERHEAD_CUPTI_INSTRUMENTATION | Instrumentation |
| 6 | OVERHEAD_RUNTIME_TRIGGERED_MODULE_LOADING | Runtime module loading |

### ENUM_CUPTI_STREAM_TYPE
| id | name | label |
|----|------|-------|
| 1 | ...DEFAULT | Default stream |
| 2 | ...NON_BLOCKING | Non-blocking stream |
| 3 | ...NULL | Null stream |

### ENUM_CUPTI_SYNC_TYPE
| id | name | label |
|----|------|-------|
| 1 | ...EVENT_SYNCHRONIZE | Event sync |
| 2 | ...STREAM_WAIT_EVENT | Stream wait sync |
| 3 | ...STREAM_SYNCHRONIZE | Stream sync |
| 4 | ...CONTEXT_SYNCHRONIZE | Context sync |

### ENUM_NSYS_EVENT_CLASS
主要值 (完整列表共 80+ 项):

| id | name | label |
|----|------|-------|
| 0 | TRACE_PROCESS_EVENT_CUDA_RUNTIME | CUDA runtime |
| 1 | TRACE_PROCESS_EVENT_CUDA_DRIVER | CUDA driver |
| 6 | TRACE_PROCESS_EVENT_NVTX_START | NVTX start |
| 7 | TRACE_PROCESS_EVENT_NVTX_FINISH | NVTX finish |
| 26 | TRACE_PROCESS_EVENT_CUDA_API | CUDA API |
| 27 | TRACE_PROCESS_EVENT_OS_RUNTIME | OSRT runtime |
| 28 | TRACE_PROCESS_EVENT_CUDNN | cuDNN |
| 29 | TRACE_PROCESS_EVENT_CUBLAS | cuBLAS |

### ENUM_NSYS_EVENT_TYPE
NVTX 相关主要值:

| id | name | label |
|----|------|-------|
| 32 | NvtxEvents | NvtxEvents |
| 35 | NvtxPushRange | NvtxPushRange |
| 36 | NvtxPopRange | NvtxPopRange |
| 37 | NvtxStartRange | NvtxStartRange |
| 38 | NvtxEndRange | NvtxEndRange |
| 79 | CudaDeviceKernel | CudaDeviceKernel |
| 80 | CudaDeviceMemory | CudaDeviceMemory |

### ENUM_DIAGNOSTIC_SEVERITY_LEVEL
| id | name | label |
|----|------|-------|
| 1 | Info | Info |
| 2 | Warning | Warning |
| 3 | Error | Error |

### ENUM_DIAGNOSTIC_SOURCE_TYPE
| id | name | label |
|----|------|-------|
| 1 | Injection | Injection |
| 2 | Daemon | Daemon |
| 3 | Analysis | Analysis |

### ENUM_DIAGNOSTIC_TIMESTAMP_SOURCE
| id | name | label |
|----|------|-------|
| 1 | TargetTimestamp | Target timestamp |
| 2 | HostTimestamp | Host timestamp |

### ENUM_GPU_CTX_SWITCH
| id | name | label |
|----|------|-------|
| 0 | SOF | SO (Start of Frame) |
| 1 | REQ_BY_HOST | Request by host |
| 2 | FE_ACK | Front-end ACK |
| 7 | SAVE_END | Save end |
| 8 | RESTORE_START | Restore start |
| 9 | CONTEXT_START | Context start |

### ENUM_SAMPLING_THREAD_STATE
| id | name | label |
|----|------|-------|
| 1 | Running | Running |
| 2 | Interruptible | Interruptible |
| 3 | Uninterruptible | Uninterruptible |

### ENUM_SCHEDULING_THREAD_BLOCK
| id | name | label |
|----|------|-------|
| 0 | NonBlocked | NonBlocked |
| 6 | Mutex | Mutex |
| 7 | CondVar | CondVar |
| 23 | DelayExecution | DelayExecution |

### ENUM_STACK_UNWIND_METHOD
| id | name | label |
|----|------|-------|
| 1 | FramePointer | Frame pointer |
| 2 | UnwindTable | Unwind table |
| 6 | DWARF_EH | DWARF EH |

### ENUM_OSRT_FILE_ACCESS_EVENT_TYPE
| id | name | label |
|----|------|-------|
| 1 | Open | Open |
| 2 | Close | Close |
| 3 | Read | Read |
| 4 | Write | Write |

### ETW 相关枚举 (Windows 专用)
- **ENUM_NSYS_GENERIC_EVENT_FIELD_ETW_FLAGS**: ETW 事件映射标志
- **ENUM_NSYS_GENERIC_EVENT_FIELD_ETW_PROPERTY**: ETW 属性标志
- **ENUM_NSYS_GENERIC_EVENT_FIELD_ETW_TYPE**: ETW 字段数据类型 (NULL/UnicodeString/ANSI String/int8~uint64/float/double/boolean/GUID 等)
- **ENUM_NSYS_GENERIC_EVENT_FIELD_TYPE**: 通用事件字段类型 (Signed/Unsigned/Float/Double)
- **ENUM_NSYS_GENERIC_EVENT_GROUP**: 事件来源组 (FTrace/NvMedia/Hypervisor/ETW)
- **ENUM_NSYS_GENERIC_EVENT_SOURCE**: 事件时间戳来源 (ClockMonotonicRaw/CntVct/SessionNs)

## 12. 常用查询示例

### 查看 Kernel 执行时间 Top 10
```sql
SELECT s.value AS name,
       (k.end - k.start) AS duration_ns,
       k.gridX, k.gridY, k.gridZ,
       k.blockX, k.blockY, k.blockZ,
       k.registersPerThread
FROM CUPTI_ACTIVITY_KIND_KERNEL k
JOIN StringIds s ON k.shortName = s.id
ORDER BY duration_ns DESC
LIMIT 10;
```

### 查看 Host ↔ Device 内存拷贝
```sql
SELECT m.start, m.end, m.bytes,
       e1.label AS src_kind, e2.label AS dst_kind,
       e3.label AS copy_kind
FROM CUPTI_ACTIVITY_KIND_MEMCPY m
LEFT JOIN ENUM_CUDA_MEM_KIND e1 ON m.srcKind = e1.id
LEFT JOIN ENUM_CUDA_MEM_KIND e2 ON m.dstKind = e2.id
LEFT JOIN ENUM_CUDA_MEMCPY_OPER e3 ON m.copyKind = e3.id;
```

### 查看 NVTX 标注区间
```sql
SELECT n.start, n.end, n.text,
       (n.end - n.start) AS duration_ns
FROM NVTX_EVENTS n
WHERE n.end IS NOT NULL
ORDER BY duration_ns DESC;
```

### 查看 GPU 显存分配/释放
```sql
SELECT e.start, s.value AS name, e.bytes,
       d.label AS operation,
       m.label AS mem_kind
FROM CUDA_GPU_MEMORY_USAGE_EVENTS e
LEFT JOIN ENUM_CUDA_DEV_MEM_EVENT_OPER d ON e.memoryOperationType = d.id
LEFT JOIN ENUM_CUDA_MEM_KIND m ON e.memKind = m.id
LEFT JOIN StringIds s ON e.name = s.value;
```

### Runtime API 与 Kernel 关联
```sql
SELECT r.start AS api_start, r.end AS api_end,
       s.value AS api_name,
       k.start AS kernel_start, k.end AS kernel_end,
       sk.value AS kernel_name
FROM CUPTI_ACTIVITY_KIND_RUNTIME r
JOIN StringIds s ON r.nameId = s.id
JOIN CUPTI_ACTIVITY_KIND_KERNEL k ON r.correlationId = k.correlationId
JOIN StringIds sk ON k.shortName = sk.id
ORDER BY r.start;
```
