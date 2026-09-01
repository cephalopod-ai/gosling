// Owns native menu label translation for Electron's main process.
// Extracted from ui/desktop/src/main.ts in a behavior-preserving modularization.
// The compatibility facade imports translateMenuLabel and translateMenuLabels; it re-exports none.

import type { MenuItem } from 'electron';

const MENU_TRANSLATIONS_ZH_CN: Record<string, string> = {
  File: '文件',
  Edit: '编辑',
  View: '视图',
  Window: '窗口',
  Help: '帮助',
  'Add to dictionary': '添加到词典',
  Cut: '剪切',
  Copy: '复制',
  Paste: '粘贴',
  'New Window': '新建窗口',
  Settings: '设置',
  'Find…': '查找…',
  'Find Next': '查找下一个',
  'Find Previous': '查找上一个',
  'Use Selection for Find': '用所选内容查找',
  Find: '查找',
  'New Chat': '新建聊天',
  'New Chat Window': '新建聊天窗口',
  'Open Directory...': '打开目录…',
  'Recent Directories': '最近的目录',
  'Focus Gosling Window': '聚焦 Gosling 窗口',
  'Quick Launcher': '快速启动器',
  'Always on Top': '窗口置顶',
  'Toggle Navigation': '切换导航',
  'About Gosling': '关于 Gosling',
  Undo: '撤销',
  Redo: '重做',
  'Select All': '全选',
  Delete: '删除',
  Speech: '语音',
  Reload: '重新加载',
  'Force Reload': '强制重新加载',
  'Toggle Developer Tools': '切换开发者工具',
  'Actual Size': '实际大小',
  'Reset Zoom': '重置缩放',
  'Zoom In': '放大',
  'Zoom Out': '缩小',
  'Toggle Full Screen': '切换全屏',
  'Toggle Fullscreen': '切换全屏',
  Minimize: '最小化',
  Close: '关闭',
  'Close Window': '关闭窗口',
  Quit: '退出',
  Exit: '退出',
  'Bring All to Front': '全部置于最前',
  'Emoji & Symbols': '表情符号',
  'Start Dictation…': '开始听写…',
  'Hide Gosling': '隐藏 Gosling',
  'Hide Others': '隐藏其他',
  'Show All': '全部显示',
  Services: '服务',
};

export function translateMenuLabel(locale: string, label: string): string {
  const lower = locale.replace(/_/g, '-').toLowerCase();
  const isTraditional = /^zh-(hant|tw|hk|mo)\b/.test(lower);
  const isSimplifiedChinese = !isTraditional && (lower === 'zh' || lower.startsWith('zh-'));
  return isSimplifiedChinese ? (MENU_TRANSLATIONS_ZH_CN[label] ?? label) : label;
}

export function translateMenuLabels(items: MenuItem[], translate: (label: string) => string): void {
  for (const item of items) {
    if (item.label) {
      const translated = translate(item.label);
      if (translated !== item.label) {
        (item as unknown as { label: string }).label = translated;
      }
    }
    if (item.submenu?.items) {
      translateMenuLabels(item.submenu.items, translate);
    }
  }
}
