package ui

func settingsHubName(tab uint8) string {
	switch tab {
	case 9:
		return "模块管理"
	case 135:
		return "灵动岛布局编辑器"
	case 0:
		return "控制中心外观"
	case 1:
		return "歌词与本地文件"
	case 2:
		return "应用通知与历史"
	case 3:
		return "关于与更新"
	case 4:
		return "启动、窗口与字体"
	case 5:
		return "设备连接提醒"
	case 6:
		return "提醒音效"
	case 7:
		return "按应用提醒"
	case 8:
		return "计时与待办偏好"
	case 128:
		return "灵动岛尺寸与排版"
	case 129:
		return "材质与取色"
	case 130:
		return "交互与系统提示"
	case 131:
		return "光效、动效与圆角"
	case 132:
		return "播放器与同步"
	case 133:
		return "播放进度条"
	case 134:
		return "播放按钮主题"
	}
	return "设置"
}
