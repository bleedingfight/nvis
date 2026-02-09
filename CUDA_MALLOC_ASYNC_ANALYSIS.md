# 分析 cudaMallocAsync 调用耗时指南

本指南介绍如何使用 Nsight Systems 和分析工具来查看 cudaMallocAsync 及其他 CUDA API 在不同时间点的调用耗时。

## 前置条件

1. 安装必要的 Python 包：
```bash
pip install pandas matplotlib
```

2. 使用 Nsight Systems 采集性能数据：
```bash
# 采集 CUDA runtime API 调用信息
nsys profile --trace=cuda,nvtx,osrt ./your_program

# 生成 SQLite 数据库（如果没有自动生成）
nsys export --type sqlite report1.nsys-rep
```

## 方法一：使用通用分析工具 (nsys_analyzer.py)

### 1. 查看所有可用的 CUDA Runtime API

```bash
python nsys_analyzer.py report1.sqlite --list --type runtime
```

输出示例：
```
              function_name  call_count    type
             cudaFree_v3020          20 runtime
           cudaMalloc_v3020          20 runtime
     cudaMallocAsync_v11020          20 runtime
           cudaMemcpy_v3020          20 runtime
```

### 2. 查询 cudaMallocAsync 的详细耗时

```bash
# 精确匹配（需要完整的函数名，包括版本号）
python nsys_analyzer.py report1.sqlite --function cudaMallocAsync_v11020 --type runtime

# 使用模糊匹配（推荐）
python nsys_analyzer.py report1.sqlite --function "MallocAsync" --pattern --type runtime
```

### 3. 只查看统计摘要

```bash
python nsys_analyzer.py report1.sqlite --function cudaMallocAsync_v11020 --type runtime --stats
```

输出示例：
```
Function: cudaMallocAsync_v11020
Total Calls: 20
Total Time: 17.11 ms
Mean Time: 855.41 μs
Median Time: 527.27 μs
Min Time: 488.12 μs
Max Time: 7132.48 μs
Std Dev: 1477.83 μs
```

### 4. 导出到 CSV 文件

```bash
python nsys_analyzer.py report1.sqlite --function cudaMallocAsync_v11020 --type runtime --output cudaMallocAsync_timing.csv
```

CSV 文件包含以下字段：
- `function_name`: 函数名
- `start_time_ns`: 开始时间（纳秒）
- `end_time_ns`: 结束时间（纳秒）
- `duration_ns/us/ms`: 持续时间（纳秒/微秒/毫秒）
- `thread_id`: 线程 ID
- `correlation_id`: 关联 ID
- `return_value`: 返回值

## 方法二：使用专用分析工具 (analyze_cudaMallocAsync.py)

这个工具专门用于分析内存分配相关的 API，提供了更丰富的分析功能。

### 1. 基础分析

```bash
python analyze_cudaMallocAsync.py report1.sqlite
```

这会自动：
- 查找所有内存分配相关的函数（cudaMalloc, cudaMallocAsync, cudaFree 等）
- 显示每个函数的统计信息
- 分析时间趋势（如前半部分 vs 后半部分的耗时变化）

输出示例：
```
cudaMallocAsync_v11020 调用统计:
  总调用次数: 20
  总耗时: 17.108 ms
  平均耗时: 855.41 μs
  最小耗时: 488.12 μs
  最大耗时: 7132.48 μs
  标准差: 1477.83 μs

时间序列分析:
  第一次调用耗时: 488.12 μs
  最后一次调用耗时: 6890.42 μs
  前半部分平均: 520.45 μs
  后半部分平均: 1190.37 μs
  ⚠ 注意: 后半部分调用耗时增加了 128.7%
```

### 2. 生成可视化图表

```bash
python analyze_cudaMallocAsync.py report1.sqlite --visualize
```

这会生成一个包含 4 个子图的综合分析图表：
1. **随时间变化的耗时趋势图**：显示每次调用的耗时随相对时间的变化
2. **按调用顺序的耗时分布**：显示第 1 次、第 2 次...第 N 次调用的耗时
3. **耗时分布直方图**：显示耗时的统计分布
4. **箱线图**：显示最小值、四分位数、中位数、最大值

### 3. 比较多个报告

```bash
python analyze_cudaMallocAsync.py report1.sqlite --compare report2.sqlite report3.sqlite
```

这会生成一个对比表，方便比较不同实验或优化版本的性能：
```
 report               function  total_calls  total_time_ms   mean_us  median_us  min_us   max_us
report1       cudaMalloc_v3020           20          7.342    367.11     198.69  124.70   628.24
report2 cudaMallocAsync_v11020           20         17.108    855.41     527.27  488.12  7132.48
report3 cudaMallocAsync_v11020           20          7.446    372.29       1.97    0.91  6890.42
```

## 实际应用场景

### 场景 1：调试内存分配性能问题

如果您发现程序运行变慢，想知道是否是内存分配导致的：

```bash
# 1. 先查看所有 runtime API 的耗时
python nsys_analyzer.py report1.sqlite --list --type runtime

# 2. 分析内存分配相关的函数
python analyze_cudaMallocAsync.py report1.sqlite --visualize

# 3. 检查是否有明显的性能下降趋势
```

### 场景 2：对比 cudaMalloc vs cudaMallocAsync

```bash
# 采集两份数据
nsys profile -o malloc_sync ./program_with_cudaMalloc
nsys profile -o malloc_async ./program_with_cudaMallocAsync

# 对比性能
python analyze_cudaMallocAsync.py malloc_sync.sqlite --compare malloc_async.sqlite
```

### 场景 3：追踪特定时间点的性能异常

如果您发现某些时间点的 cudaMallocAsync 调用特别慢：

```bash
# 导出详细数据到 CSV
python nsys_analyzer.py report1.sqlite --function cudaMallocAsync_v11020 --type runtime --output timing.csv

# 在 CSV 中查找：
# - start_time_ns: 找到具体的时间点
# - duration_us: 找到异常慢的调用
# - correlation_id: 用于关联到具体的代码位置
```

然后在 Nsight Systems GUI 中使用 correlation_id 定位到具体的代码位置。

## 高级分析

### 使用 Python 脚本进行自定义分析

```python
from nsys_analyzer import NsysAnalyzer

# 连接数据库
analyzer = NsysAnalyzer('report1.sqlite')
analyzer.connect()

# 查询数据
df = analyzer.query_function_timing('cudaMallocAsync_v11020', 
                                   use_pattern=False, 
                                   api_type='runtime')

# 自定义分析
import matplotlib.pyplot as plt

# 例如：分析调用间隔时间
df['interval_ms'] = df['start_time_ns'].diff() / 1e6

print(f"平均调用间隔: {df['interval_ms'].mean():.2f} ms")
print(f"最小调用间隔: {df['interval_ms'].min():.2f} ms")
print(f"最大调用间隔: {df['interval_ms'].max():.2f} ms")

# 可视化
plt.figure(figsize=(10, 6))
plt.scatter(df['interval_ms'][1:], df['duration_us'][1:])
plt.xlabel('Time since last call (ms)')
plt.ylabel('Duration (μs)')
plt.title('Memory allocation duration vs. call interval')
plt.savefig('custom_analysis.png')
```

## 常见问题

### Q1: 找不到 cudaMallocAsync 函数？

**A:** 可能的原因：
1. 您的程序没有使用 cudaMallocAsync（使用的是传统的 cudaMalloc）
2. Nsight Systems 没有采集 runtime API 数据

解决方法：
```bash
# 检查是否采集了 runtime API
python nsys_analyzer.py report1.sqlite --list --type runtime

# 如果没有数据，重新采集时加上 --trace 参数
nsys profile --trace=cuda,nvtx,osrt ./your_program
```

### Q2: 如何知道 cudaMallocAsync 的完整函数名（包括版本号）？

**A:** 使用模糊匹配：
```bash
python nsys_analyzer.py report1.sqlite --list --type runtime --pattern
# 或者直接搜索
python nsys_analyzer.py report1.sqlite --function "Async" --pattern --type runtime
```

### Q3: 为什么 cudaMallocAsync 耗时波动很大？

**A:** 可能的原因：
1. **第一次调用开销**：首次分配可能需要初始化内存池
2. **内存碎片**：长时间运行后内存池碎片化
3. **同步操作**：某些情况下需要等待 GPU 空闲
4. **竞争条件**：多线程同时调用时的锁竞争

可以通过可视化图表来识别这些模式：
```bash
python analyze_cudaMallocAsync.py report1.sqlite --visualize
```

### Q4: 如何关联到源代码位置？

**A:** 使用 correlation_id：
1. 在 CSV 导出中找到 correlation_id
2. 在 Nsight Systems GUI 中打开 report.nsys-rep
3. 使用 correlation_id 在 Events View 中搜索
4. 双击事件即可跳转到调用栈

## 性能优化建议

基于分析结果，以下是一些优化建议：

1. **如果平均耗时过高（> 1ms）**：
   - 考虑批量分配而不是频繁小分配
   - 使用内存池预分配
   - 检查是否有不必要的同步

2. **如果后期耗时明显增加**：
   - 可能存在内存碎片问题
   - 考虑定期重建内存池
   - 检查是否有内存泄漏

3. **如果波动很大（std > mean/2）**：
   - 使用异步流避免同步等待
   - 分析是否与 kernel 执行冲突
   - 考虑固定内存分配策略

## 参考命令速查表

```bash
# 列出所有 runtime API
python nsys_analyzer.py report.sqlite --list --type runtime

# 查询 cudaMallocAsync
python nsys_analyzer.py report.sqlite -f cudaMallocAsync -p -t runtime

# 快速统计
python nsys_analyzer.py report.sqlite -f cudaMallocAsync -p -t runtime -s

# 导出 CSV
python nsys_analyzer.py report.sqlite -f cudaMallocAsync -p -t runtime -o output.csv

# 可视化分析
python analyze_cudaMallocAsync.py report.sqlite --visualize

# 比较多个报告
python analyze_cudaMallocAsync.py report1.sqlite -c report2.sqlite report3.sqlite
```

## 总结

通过这些工具，您可以：
- ✅ 查看 cudaMallocAsync 在不同时间点的调用耗时
- ✅ 分析性能趋势和异常
- ✅ 对比不同实现或优化的效果
- ✅ 导出数据进行自定义分析
- ✅ 生成可视化报告

如果有任何问题或需要更多功能，欢迎反馈！
