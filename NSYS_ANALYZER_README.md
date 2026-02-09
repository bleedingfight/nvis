# NVIDIA Nsight Systems (nsys) 分析工具

这是一套用于分析NVIDIA Nsight Systems采集数据的Python工具，可以查询特定函数在不同时间点的调用耗时。

## 文件说明

- `nsys_analyzer.py` - 主要分析工具，用于查询和统计函数耗时
- `visualize_nsys.py` - 可视化工具，生成函数耗时的趋势图
- `timing_results.csv` - 导出的示例结果文件
- `timing_plot.png` - 生成的示例可视化图表

## 依赖安装

```bash
# 基础依赖
pip install pandas

# 如果需要可视化功能
pip install matplotlib
```

## 使用方法

### 1. 查看所有可用的函数

```bash
python nsys_analyzer.py report1.sqlite --list
```

### 2. 查询特定函数的详细耗时

```bash
# 精确匹配函数名
python nsys_analyzer.py report1.sqlite --function vector_add_scalar_kernel

# 使用模糊匹配
python nsys_analyzer.py report1.sqlite --function "vector_add" --pattern
```

### 3. 只显示统计信息

```bash
python nsys_analyzer.py report1.sqlite --function vector_add_scalar_kernel --stats
```

输出示例：
```
Function: vector_add_scalar_kernel
Total Calls: 10
Total Time: 3.87 ms
Mean Time: 386.71 μs
Median Time: 386.78 μs
Min Time: 386.21 μs
Max Time: 387.23 μs
Std Dev: 0.34 μs
```

### 4. 导出结果到CSV文件

```bash
python nsys_analyzer.py report1.sqlite --function vector_add_scalar_kernel --output results.csv
```

CSV文件包含以下字段：
- `function_name` - 函数名
- `start_time_ns` - 开始时间（纳秒）
- `end_time_ns` - 结束时间（纳秒）
- `duration_ns` - 持续时间（纳秒）
- `duration_us` - 持续时间（微秒）
- `duration_ms` - 持续时间（毫秒）
- `device_id` - GPU设备ID
- `stream_id` - CUDA流ID
- `grid_size` - 网格大小
- `block_size` - 块大小

### 5. 生成可视化图表

```bash
# 生成单个函数的耗时趋势图
python visualize_nsys.py report1.sqlite --function vector_add_scalar_kernel --output timing.png

# 比较多个函数的耗时
python visualize_nsys.py report1.sqlite --compare func1 func2 func3 --output comparison.png
```

生成的图表包括：
- 函数调用耗时随时间的变化趋势
- 耗时分布直方图
- 统计信息（均值、中位数、最小值、最大值、标准差）

## 工作原理

1. **数据源**: nsys采集的数据会被导出为SQLite数据库文件（`.sqlite`）
2. **关键表**: 工具主要查询以下表：
   - `CUPTI_ACTIVITY_KIND_KERNEL` - GPU kernel执行信息
   - `CUPTI_ACTIVITY_KIND_RUNTIME` - CUDA runtime API调用信息
   - `StringIds` - 字符串ID映射表

3. **查询逻辑**: 
   - 通过JOIN操作关联kernel表和字符串表
   - 按开始时间排序，获取每次调用的时间戳
   - 计算duration = end_time - start_time

## 示例输出

### 详细耗时信息
```
           function_name  start_time_ns  end_time_ns  duration_ns  duration_us  duration_ms
vector_add_scalar_kernel     1238832590   1239218799       386209      386.209     0.386209
vector_add_scalar_kernel     1428363145   1428749450       386305      386.305     0.386305
...
```

### 统计摘要
```
Total Calls: 10
Total Time: 3.87 ms
Mean Time: 386.71 μs
Median Time: 386.78 μs
Min Time: 386.21 μs
Max Time: 387.23 μs
Std Dev: 0.34 μs
```

## 进阶用法

### 分析不同报告文件

```bash
# 比较不同实验的结果
python nsys_analyzer.py report1.sqlite --function my_kernel --stats > report1_stats.txt
python nsys_analyzer.py report2.sqlite --function my_kernel --stats > report2_stats.txt
python nsys_analyzer.py report3.sqlite --function my_kernel --stats > report3_stats.txt
```

### 批量分析

创建一个脚本来批量分析多个函数：

```bash
#!/bin/bash
FUNCTIONS=("kernel1" "kernel2" "kernel3")

for func in "${FUNCTIONS[@]}"; do
    echo "Analyzing $func..."
    python nsys_analyzer.py report1.sqlite --function "$func" --output "${func}_timing.csv"
done
```

## 注意事项

1. 确保SQLite数据库文件存在且格式正确
2. 函数名区分大小写（除非使用`--pattern`参数）
3. 时间戳单位为纳秒（ns），工具会自动转换为微秒（μs）和毫秒（ms）
4. 大型数据集可能需要较长的查询时间

## 获取帮助

```bash
# 查看nsys_analyzer的帮助信息
python nsys_analyzer.py --help

# 查看visualize_nsys的帮助信息
python visualize_nsys.py --help
```

## 常见问题

### Q: 如何生成SQLite数据库？
A: nsys会自动生成，如果只有`.nsys-rep`文件，可以使用：
```bash
nsys export --type sqlite report1.nsys-rep
```

### Q: 找不到我的函数？
A: 首先使用`--list`参数查看所有可用函数，然后使用`--pattern`进行模糊匹配

### Q: 支持哪些类型的函数？
A: 目前支持CUDA kernel函数，如需分析CPU函数或CUDA runtime API，需要修改SQL查询

## 扩展功能

如需添加更多分析功能，可以修改SQL查询来获取：
- Memory transfer分析（`CUPTI_ACTIVITY_KIND_MEMCPY`表）
- CUDA runtime API调用（`CUPTI_ACTIVITY_KIND_RUNTIME`表）
- 同步事件分析（`CUPTI_ACTIVITY_KIND_SYNCHRONIZATION`表）
