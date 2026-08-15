use console::style;

/// Nord 配色（https://www.nordtheme.com/）
/// Frost 系：nord9=#81A1C1 蓝, nord10=#5E81AC 深蓝
/// Aurora 系：nord11=#BF616A 红, nord12=#D08770 橙, nord13=#EBCB8B 黄,
///            nord14=#A3BE8C 绿, nord15=#B48EAD 紫
pub const NORD9: (u8, u8, u8) = (129, 161, 193); // frost 蓝
pub const NORD10: (u8, u8, u8) = (94, 129, 172); // frost 深蓝
pub const NORD11: (u8, u8, u8) = (191, 97, 106); // aurora 红
pub const NORD12: (u8, u8, u8) = (208, 135, 112); // aurora 橙
pub const NORD13: (u8, u8, u8) = (235, 203, 139); // aurora 黄
pub const NORD14: (u8, u8, u8) = (163, 190, 140); // aurora 绿
pub const NORD15: (u8, u8, u8) = (180, 142, 173); // aurora 紫

/// 标题（nord15 紫加粗，醒目）
pub fn title(text: &str) -> String {
    style(text).true_color(NORD15.0, NORD15.1, NORD15.2).bold().to_string()
}

/// 提示语（nord9 蓝）
pub fn prompt(text: &str) -> String {
    style(text).true_color(NORD9.0, NORD9.1, NORD9.2).to_string()
}

/// 菜单标题（nord10 深蓝加粗）
pub fn menu_title(text: &str) -> String {
    style(text).true_color(NORD10.0, NORD10.1, NORD10.2).bold().to_string()
}

/// 选项序号（nord12 橙）
pub fn option(text: &str) -> String {
    style(text).true_color(NORD12.0, NORD12.1, NORD12.2).to_string()
}

/// 成功信息（nord14 绿）
pub fn success(text: &str) -> String {
    style(text).true_color(NORD14.0, NORD14.1, NORD14.2).to_string()
}

/// 成功符号（nord14 绿加粗）
pub fn success_mark(text: &str) -> String {
    style(text).true_color(NORD14.0, NORD14.1, NORD14.2).bold().to_string()
}

/// 失败信息（nord11 红）
pub fn failure(text: &str) -> String {
    style(text).true_color(NORD11.0, NORD11.1, NORD11.2).to_string()
}

/// 失败符号（nord11 红加粗）
pub fn failure_mark(text: &str) -> String {
    style(text).true_color(NORD11.0, NORD11.1, NORD11.2).bold().to_string()
}

/// 取消符号（nord13 黄加粗）
pub fn cancel_mark(text: &str) -> String {
    style(text).true_color(NORD13.0, NORD13.1, NORD13.2).bold().to_string()
}
