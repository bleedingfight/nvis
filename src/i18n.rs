/// 国际化(i18n)支持模块
/// 支持多语言切换，默认英文，可切换为中文

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    pub fn all_languages() -> Vec<Language> {
        vec![Language::English, Language::Chinese]
    }

    pub fn name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Chinese => "中文",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Chinese => "zh",
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::English
    }
}

/// 文本翻译键
#[allow(dead_code)]
pub enum TextKey {
    // 应用标题
    AppTitle,

    // 顶部面板
    FileLabel,
    DragDropHint,
    LogSettings,
    LanguageSettings,

    // 左侧面板
    DataTables,
    NoData,
    Reload,

    // 中央面板 - 表格视图
    Table,
    Visualization,
    BackToTimeline,
    TableData,
    DataVisualization,
    ResetView,
    ResetZoom,
    PageNumber,
    PreviousPage,
    NextPage,
    RowsPerPage,
    TotalPages,
    TotalRows,
    Loading,

    // 中央面板 - 时间线
    Timeline,
    NoFunctionCallData,
    DragDropPrompt,

    // 底部状态栏
    Error,
    SelectedTimeRange,
    ClearSelection,
    FunctionCalls,
    LogLevel,

    // 日志设置窗口
    LogLevelTitle,
    LogLevelOff,
    LogLevelError,
    LogLevelWarn,
    LogLevelInfo,
    LogLevelDebug,
    LogLevelTrace,
    LogDescription,
    LogDescOff,
    LogDescError,
    LogDescWarn,
    LogDescInfo,
    LogDescDebug,
    LogDescTrace,
    Close,

    // 语言设置窗口
    LanguageTitle,
    LanguageDescription,

    // 可视化
    SelectTableHint,
    DataSummary,
    Columns,
    Rows,
    NumericColumnsHint,
    BarChart,
    ScatterPlot,
    DataRange,
    Points,
    ZoomControl,
    MouseWheelZoom,
    DragToMove,

    // 错误消息
    LoadDatabaseFailed,
    LoadTableDataFailed,
    LoadPageFailed,
    NoFunctionCallDataFound,
    InvalidNsysFile,

    // 面板提示
    EnableAtLeastOnePanel,
    ClickToEnablePanel,
}

impl TextKey {
    pub fn get(self, lang: Language) -> &'static str {
        match lang {
            Language::English => self.en(),
            Language::Chinese => self.zh(),
        }
    }

    fn en(self) -> &'static str {
        match self {
            // 应用标题
            TextKey::AppTitle => "NSYS Profile Viewer",

            // 顶部面板
            TextKey::FileLabel => "File:",
            TextKey::DragDropHint => "Drag and drop SQLite file here",
            TextKey::LogSettings => "⚙ Log Settings",
            TextKey::LanguageSettings => "🌐 Language",

            // 左侧面板
            TextKey::DataTables => "Data Tables",
            TextKey::NoData => "No Data",
            TextKey::Reload => "Reload",

            // 中央面板 - 表格视图
            TextKey::Table => "Table",
            TextKey::Visualization => "Visualization",
            TextKey::BackToTimeline => "Back to Timeline",
            TextKey::TableData => "📊 Table Data",
            TextKey::DataVisualization => "📈 Data Visualization",
            TextKey::ResetView => "🔄 Reset View",
            TextKey::ResetZoom => "🔄 Reset Zoom",
            TextKey::PageNumber => "Page:",
            TextKey::PreviousPage => "⏮ Previous",
            TextKey::NextPage => "Next ⏭",
            TextKey::RowsPerPage => "Rows per page:",
            TextKey::TotalPages => "Total pages:",
            TextKey::TotalRows => "Total rows:",
            TextKey::Loading => "Loading...",

            // 中央面板 - 时间线
            TextKey::Timeline => "Timeline",
            TextKey::NoFunctionCallData => "No function call data",
            TextKey::DragDropPrompt => "Please drag and drop a SQLite file with profiling data",

            // 底部状态栏
            TextKey::Error => "Error:",
            TextKey::SelectedTimeRange => "Selected time range:",
            TextKey::ClearSelection => "Clear Selection",
            TextKey::FunctionCalls => "Function calls:",
            TextKey::LogLevel => "Log:",

            // 日志设置窗口
            TextKey::LogLevelTitle => "Log Level",
            TextKey::LogLevelOff => "Off",
            TextKey::LogLevelError => "Error",
            TextKey::LogLevelWarn => "Warning",
            TextKey::LogLevelInfo => "Info",
            TextKey::LogLevelDebug => "Debug",
            TextKey::LogLevelTrace => "Trace",
            TextKey::LogDescription => "Description:",
            TextKey::LogDescOff => "• Off: No logging",
            TextKey::LogDescError => "• Error: Only error messages",
            TextKey::LogDescWarn => "• Warning: Warnings and errors",
            TextKey::LogDescInfo => "• Info: General information",
            TextKey::LogDescDebug => "• Debug: Debug information (recommended)",
            TextKey::LogDescTrace => "• Trace: Detailed trace information",
            TextKey::Close => "Close",

            // 语言设置窗口
            TextKey::LanguageTitle => "Language Settings",
            TextKey::LanguageDescription => "Select your preferred language:",

            // 可视化
            TextKey::SelectTableHint => "Select a data table to display visualization",
            TextKey::DataSummary => "Data Summary",
            TextKey::Columns => "Columns:",
            TextKey::Rows => "Rows:",
            TextKey::NumericColumnsHint => "Tip: Numeric columns are needed to generate charts",
            TextKey::BarChart => "Bar Chart",
            TextKey::ScatterPlot => "Scatter Plot",
            TextKey::DataRange => "Range:",
            TextKey::Points => "Points:",
            TextKey::ZoomControl => "Zoom:",
            TextKey::MouseWheelZoom => "Mouse wheel to zoom",
            TextKey::DragToMove => "Drag to move",

            // 错误消息
            TextKey::LoadDatabaseFailed => "Failed to load database:",
            TextKey::LoadTableDataFailed => "Failed to load table data:",
            TextKey::LoadPageFailed => "Failed to load page:",
            TextKey::NoFunctionCallDataFound => "No function call data found",
            TextKey::InvalidNsysFile => "Please ensure this is a valid NSYS output file",

            // 面板提示
            TextKey::EnableAtLeastOnePanel => "Please enable at least one panel",
            TextKey::ClickToEnablePanel => "Click 'Table' or 'Visualization' button above",
        }
    }

    fn zh(self) -> &'static str {
        match self {
            // 应用标题
            TextKey::AppTitle => "NSYS 性能分析查看器",

            // 顶部面板
            TextKey::FileLabel => "文件:",
            TextKey::DragDropHint => "拖放 SQLite 文件到窗口",
            TextKey::LogSettings => "⚙ 日志设置",
            TextKey::LanguageSettings => "🌐 语言",

            // 左侧面板
            TextKey::DataTables => "数据表",
            TextKey::NoData => "暂无数据",
            TextKey::Reload => "重新加载",

            // 中央面板 - 表格视图
            TextKey::Table => "表格",
            TextKey::Visualization => "可视化",
            TextKey::BackToTimeline => "返回时间线",
            TextKey::TableData => "📊 表格数据",
            TextKey::DataVisualization => "📈 数据可视化",
            TextKey::ResetView => "🔄 重置视图",
            TextKey::ResetZoom => "🔄 重置缩放",
            TextKey::PageNumber => "页码:",
            TextKey::PreviousPage => "⏮ 上一页",
            TextKey::NextPage => "下一页 ⏭",
            TextKey::RowsPerPage => "每页显示:",
            TextKey::TotalPages => "总页数:",
            TextKey::TotalRows => "总行数:",
            TextKey::Loading => "正在加载...",

            // 中央面板 - 时间线
            TextKey::Timeline => "时间线",
            TextKey::NoFunctionCallData => "没有函数调用数据",
            TextKey::DragDropPrompt => "请拖放一个包含性能分析数据的 SQLite 文件",

            // 底部状态栏
            TextKey::Error => "错误:",
            TextKey::SelectedTimeRange => "选中时间范围:",
            TextKey::ClearSelection => "清除选择",
            TextKey::FunctionCalls => "函数调用:",
            TextKey::LogLevel => "日志:",

            // 日志设置窗口
            TextKey::LogLevelTitle => "日志等级",
            TextKey::LogLevelOff => "关闭",
            TextKey::LogLevelError => "错误",
            TextKey::LogLevelWarn => "警告",
            TextKey::LogLevelInfo => "信息",
            TextKey::LogLevelDebug => "调试",
            TextKey::LogLevelTrace => "追踪",
            TextKey::LogDescription => "说明:",
            TextKey::LogDescOff => "• 关闭: 不记录任何日志",
            TextKey::LogDescError => "• 错误: 仅记录错误信息",
            TextKey::LogDescWarn => "• 警告: 记录警告和错误",
            TextKey::LogDescInfo => "• 信息: 记录一般信息",
            TextKey::LogDescDebug => "• 调试: 记录调试信息(推荐)",
            TextKey::LogDescTrace => "• 追踪: 记录详细追踪信息",
            TextKey::Close => "关闭",

            // 语言设置窗口
            TextKey::LanguageTitle => "语言设置",
            TextKey::LanguageDescription => "选择您的首选语言:",

            // 可视化
            TextKey::SelectTableHint => "选择数据表以显示可视化",
            TextKey::DataSummary => "数据摘要",
            TextKey::Columns => "列数:",
            TextKey::Rows => "行数:",
            TextKey::NumericColumnsHint => "提示: 需要数值类型的列才能生成图表",
            TextKey::BarChart => "柱状图",
            TextKey::ScatterPlot => "散点图",
            TextKey::DataRange => "范围:",
            TextKey::Points => "点数:",
            TextKey::ZoomControl => "缩放:",
            TextKey::MouseWheelZoom => "鼠标滚轮缩放",
            TextKey::DragToMove => "拖拽移动",

            // 错误消息
            TextKey::LoadDatabaseFailed => "加载数据库失败:",
            TextKey::LoadTableDataFailed => "加载表数据失败:",
            TextKey::LoadPageFailed => "加载页面失败:",
            TextKey::NoFunctionCallDataFound => "未找到函数调用数据",
            TextKey::InvalidNsysFile => "请确保这是一个有效的 NSYS 输出文件",

            // 面板提示
            TextKey::EnableAtLeastOnePanel => "请至少启用一个面板",
            TextKey::ClickToEnablePanel => "点击上方的「表格」或「可视化」按钮",
        }
    }
}

/// 便捷翻译函数
pub fn t(key: TextKey, lang: Language) -> &'static str {
    key.get(lang)
}

/// 格式化翻译文本（用于需要插入变量的文本）
#[macro_export]
macro_rules! tf {
    ($lang:expr, $key:expr, $($arg:tt)*) => {
        format!("{} {}", $crate::i18n::t($key, $lang), format!($($arg)*))
    };
}
