# 输入框光标导航功能 - v0.2.1

## 问题

在文件路径输入框中，无法使用左右箭头键移动光标，导致编辑路径时只能删除重输。

## 解决方案

实现了完整的光标导航功能，区分输入状态和非输入状态：

### 新增功能

#### 在输入框中（输入状态）：

| 按键 | 功能 |
|------|------|
| `←` | 光标向左移动一个字符 |
| `→` | 光标向右移动一个字符 |
| `Home` | 光标移到行首 |
| `End` | 光标移到行尾 |
| `Backspace` | 删除光标前一个字符 |
| `Delete` | 删除光标后一个字符 |

#### 在其他界面（非输入状态）：

| 按键 | 功能 |
|------|------|
| `←` | 切换到左侧面板 |
| `→` | 切换到右侧面板 |
| `↑/↓` | 滚动列表/数据 |

## 技术实现

### 1. 主事件处理 (main.rs:65-82)

```rust
KeyCode::Left => {
    if app.is_inputting() {
        app.on_input_left();
    } else {
        app.on_left();
    }
}
KeyCode::Right => {
    if app.is_inputting() {
        app.on_input_right();
    } else {
        app.on_right();
    }
}
KeyCode::Home => app.on_home(),
KeyCode::End => app.on_end(),
KeyCode::Delete => app.on_delete(),
```

### 2. 应用逻辑 (app.rs:169-206)

新增方法：
- `on_delete()` - 删除光标后的字符
- `on_input_left()` - 光标左移
- `on_input_right()` - 光标右移  
- `on_home()` - 光标移到开头
- `on_end()` - 光标移到末尾

所有方法都使用 `tui_input::InputRequest` 来操作输入框：

```rust
pub fn on_input_left(&mut self) {
    if self.state == AppState::FileSelection && self.focus == Focus::FileInput {
        self.file_input
            .handle(tui_input::InputRequest::GoToPrevChar);
    }
}
```

### 3. UI光标显示 (ui.rs:51-57)

修复光标位置计算，使用 `tui-input` 的 cursor 方法：

```rust
if app.focus == Focus::FileInput {
    let cursor_pos = app.file_input.cursor();
    f.set_cursor_position((
        chunks[1].x + cursor_pos as u16 + 1,
        chunks[1].y + 1,
    ));
}
```

**之前的问题：**
```rust
// 只能显示在文本末尾
chunks[1].x + input_text.len() as u16 + 1
```

**修复后：**
```rust
// 显示在实际光标位置
chunks[1].x + cursor_pos as u16 + 1
```

## 使用示例

### 编辑文件路径

**场景：** 输入了 `/home/user/data/file.db` 但想改成 `/home/user/test/file.db`

**操作步骤：**
1. 按 `Home` 跳到开头
2. 按 `→` 移到 `/home/user/` 后面
3. 按 `Delete` 删除 `d`，继续删除 `ata/`
4. 输入 `test/`
5. 完成！

**之前：** 需要删除整个路径重新输入

**现在：** 可以精确定位并修改

### 快速导航

| 操作 | 按键组合 |
|------|---------|
| 跳到开头 | `Home` |
| 跳到末尾 | `End` |
| 删除一个单词 | `Ctrl+Backspace` (终端支持的话) |
| 左移一个字符 | `←` |
| 右移一个字符 | `→` |

## 改进效果

### 之前 (v0.2.0)

- ❌ 无法移动光标
- ❌ 只能在末尾编辑
- ❌ 修改中间内容需要全部删除
- ❌ 光标总是在末尾

### 之后 (v0.2.1)

- ✅ 完整的光标导航
- ✅ 可以在任意位置编辑
- ✅ 精确修改任意字符
- ✅ 光标跟随实际位置
- ✅ 支持 Home/End 快速跳转
- ✅ 支持 Delete 向前删除

## 兼容性

- 所有功能在输入状态下自动激活
- 非输入状态下保持原有行为
- 不影响其他界面的键盘操作
- 完全向后兼容

## 测试验证

✅ 光标左右移动
✅ Home/End 跳转
✅ Backspace 向后删除
✅ Delete 向前删除
✅ 光标位置正确显示
✅ 非输入状态不受影响
✅ 输入状态检测正确

## 版本信息

- **修复版本**: 0.2.1
- **修复日期**: 2026-02-10
- **影响文件**: 
  - `src/main.rs` (新增按键处理)
  - `src/app.rs` (新增5个方法)
  - `src/ui.rs` (修复光标位置)
  - 文档更新

## 相关问题

此修复解决了：
- Issue: 输入框中无法移动光标
- Issue: 无法编辑路径中间部分
- Issue: 光标位置显示不正确

## 后续改进建议

考虑添加：
- [ ] `Ctrl+←/→` 按单词移动
- [ ] `Ctrl+K` 删除到行尾
- [ ] `Ctrl+U` 删除到行首
- [ ] 路径自动补全
- [ ] 历史记录（上下箭头）
