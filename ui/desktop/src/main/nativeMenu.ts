/**
 * Localizes and configures the Electron application menu.
 *
 * Extracted from ui/desktop/src/main.ts during behavior-preserving modularization.
 * The Electron entrypoint remains the compatibility facade and supplies window
 * actions so this module owns no startup or window lifecycle state.
 */
import { app, BrowserWindow, Menu, MenuItem } from 'electron';
import type { OpenDialogReturnValue } from 'electron';
import type { Settings } from '../utils/settings';
import { getKeyboardShortcuts } from '../utils/settings';
import { rendererEventChannels } from '../ipc/channels';

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

export function createMenuTranslator(detectMenuLocale: () => string): {
  menuT: (label: string) => string;
  translateMenuLabels: (items: MenuItem[]) => void;
} {
  const menuT = (label: string): string => {
    const lower = detectMenuLocale().replace(/_/g, '-').toLowerCase();
    const isTraditional = /^zh-(hant|tw|hk|mo)\b/.test(lower);
    const isSimplifiedChinese = !isTraditional && (lower === 'zh' || lower.startsWith('zh-'));
    return isSimplifiedChinese ? (MENU_TRANSLATIONS_ZH_CN[label] ?? label) : label;
  };

  const translateMenuLabels = (items: MenuItem[]): void => {
    for (const item of items) {
      if (item.label) {
        const translated = menuT(item.label);
        if (translated !== item.label) {
          (item as unknown as { label: string }).label = translated;
        }
      }
      if (item.submenu?.items) translateMenuLabels(item.submenu.items);
    }
  };

  return { menuT, translateMenuLabels };
}

interface ApplicationMenuOptions {
  settings: Settings;
  version?: string;
  menuT: (label: string) => string;
  translateMenuLabels: (items: MenuItem[]) => void;
  buildRecentFilesMenu: () => Array<{ label: string; click: () => Promise<void> }>;
  createLauncher: () => BrowserWindow;
  createNewWindow: () => Promise<BrowserWindow | undefined>;
  focusWindow: () => void;
  openDirectoryDialog: () => Promise<OpenDialogReturnValue>;
}

export function configureApplicationMenu(options: ApplicationMenuOptions): void {
  const {
    settings,
    version,
    menuT,
    translateMenuLabels,
    buildRecentFilesMenu,
    createLauncher,
    createNewWindow,
    focusWindow,
    openDirectoryDialog,
  } = options;

  if (process.platform === 'darwin') {
    app.dock?.setMenu(
      Menu.buildFromTemplate([{ label: menuT('New Window'), click: () => createNewWindow() }])
    );
  }

  const menu = Menu.getApplicationMenu();
  const shortcuts = getKeyboardShortcuts(settings);
  const appMenu = menu?.items.find((item) => item.label === 'Gosling');
  if (appMenu?.submenu) {
    appMenu.submenu.insert(1, new MenuItem({ type: 'separator' }));
    if (shortcuts.settings) {
      appMenu.submenu.insert(
        1,
        new MenuItem({
          label: menuT('Settings'),
          accelerator: shortcuts.settings,
          click() {
            BrowserWindow.getFocusedWindow()?.webContents.send(
              rendererEventChannels.setView,
              'settings'
            );
          },
        })
      );
    }
    appMenu.submenu.insert(1, new MenuItem({ type: 'separator' }));
  }

  const editMenu = menu?.items.find((item) => item.label === 'Edit');
  if (editMenu?.submenu) {
    const selectAllIndex = editMenu.submenu.items.findIndex((item) => item.label === 'Select All');
    const findSubmenu = Menu.buildFromTemplate([
      {
        label: menuT('Find…'),
        accelerator: shortcuts.find || undefined,
        click: () =>
          BrowserWindow.getFocusedWindow()?.webContents.send(rendererEventChannels.findCommand),
      },
      {
        label: menuT('Find Next'),
        accelerator: shortcuts.findNext || undefined,
        click: () =>
          BrowserWindow.getFocusedWindow()?.webContents.send(rendererEventChannels.findNext),
      },
      {
        label: menuT('Find Previous'),
        accelerator: shortcuts.findPrevious || undefined,
        click: () =>
          BrowserWindow.getFocusedWindow()?.webContents.send(rendererEventChannels.findPrevious),
      },
      {
        label: menuT('Use Selection for Find'),
        accelerator: process.platform === 'darwin' ? 'Command+E' : undefined,
        click: () =>
          BrowserWindow.getFocusedWindow()?.webContents.send(
            rendererEventChannels.useSelectionFind
          ),
        visible: process.platform === 'darwin',
      },
    ]);
    editMenu.submenu.insert(
      selectAllIndex + 1,
      new MenuItem({ label: menuT('Find'), submenu: findSubmenu })
    );
  }

  const fileMenu = menu?.items.find((item) => item.label === 'File');
  if (fileMenu?.submenu) {
    let menuIndex = 0;
    if (shortcuts.newChat) {
      fileMenu.submenu.insert(
        menuIndex++,
        new MenuItem({
          label: menuT('New Chat'),
          accelerator: shortcuts.newChat,
          click: () =>
            BrowserWindow.getFocusedWindow()?.webContents.send(rendererEventChannels.newChat),
        })
      );
    }
    if (shortcuts.newChatWindow) {
      fileMenu.submenu.insert(
        menuIndex++,
        new MenuItem({
          label: menuT('New Chat Window'),
          accelerator: shortcuts.newChatWindow,
          click: () => {
            void createNewWindow();
          },
        })
      );
    }
    if (shortcuts.openDirectory) {
      fileMenu.submenu.insert(
        menuIndex++,
        new MenuItem({
          label: menuT('Open Directory...'),
          accelerator: shortcuts.openDirectory,
          click: () => openDirectoryDialog(),
        })
      );
    }
    const recentFilesSubmenu = buildRecentFilesMenu();
    if (recentFilesSubmenu.length > 0) {
      fileMenu.submenu.insert(
        menuIndex++,
        new MenuItem({ label: menuT('Recent Directories'), submenu: recentFilesSubmenu })
      );
    }
    fileMenu.submenu.insert(menuIndex++, new MenuItem({ type: 'separator' }));
    if (shortcuts.focusWindow)
      fileMenu.submenu.append(
        new MenuItem({
          label: menuT('Focus Gosling Window'),
          accelerator: shortcuts.focusWindow,
          click: focusWindow,
        })
      );
    if (shortcuts.quickLauncher)
      fileMenu.submenu.append(
        new MenuItem({
          label: menuT('Quick Launcher'),
          accelerator: shortcuts.quickLauncher,
          click: createLauncher,
        })
      );
  }

  if (menu) {
    let windowMenu = menu.items.find((item) => item.label === 'Window');
    if (!windowMenu) {
      windowMenu = new MenuItem({ label: menuT('Window'), submenu: Menu.buildFromTemplate([]) });
      const helpMenuIndex = menu.items.findIndex((item) => item.label === 'Help');
      if (helpMenuIndex >= 0) menu.items.splice(helpMenuIndex, 0, windowMenu);
      else menu.items.push(windowMenu);
    }
    if (windowMenu.submenu && shortcuts.alwaysOnTop) {
      windowMenu.submenu.append(
        new MenuItem({
          label: menuT('Always on Top'),
          type: 'checkbox',
          accelerator: shortcuts.alwaysOnTop,
          click(menuItem) {
            const focusedWindow = BrowserWindow.getFocusedWindow();
            if (!focusedWindow) return;
            const isAlwaysOnTop = menuItem.checked;
            if (process.platform === 'darwin')
              focusedWindow.setAlwaysOnTop(isAlwaysOnTop, 'floating');
            else focusedWindow.setAlwaysOnTop(isAlwaysOnTop);
            console.log(
              `[Main] Set always-on-top to ${isAlwaysOnTop} for window ${focusedWindow.id}`
            );
          },
        })
      );
    }
    const viewMenu = menu.items.find((item) => item.label === 'View');
    if (viewMenu?.submenu && shortcuts.toggleNavigation) {
      viewMenu.submenu.append(new MenuItem({ type: 'separator' }));
      viewMenu.submenu.append(
        new MenuItem({
          label: menuT('Toggle Navigation'),
          accelerator: shortcuts.toggleNavigation,
          click: () =>
            BrowserWindow.getFocusedWindow()?.webContents.send(
              rendererEventChannels.toggleNavigation
            ),
        })
      );
    }
  }

  if (menu && process.platform !== 'darwin') {
    let helpMenu = menu.items.find((item) => item.label === 'Help');
    if (!helpMenu) {
      helpMenu = new MenuItem({ label: menuT('Help'), submenu: Menu.buildFromTemplate([]) });
      menu.items.splice(menu.items.length > 0 ? menu.items.length - 1 : 0, 0, helpMenu);
    }
    if (helpMenu.submenu) {
      if (helpMenu.submenu.items.length > 0)
        helpMenu.submenu.append(new MenuItem({ type: 'separator' }));
      const about = new MenuItem({
        label: menuT('About Gosling'),
        submenu: Menu.buildFromTemplate([]),
      });
      about.submenu?.append(
        new MenuItem({ label: `Gosling v${version || app.getVersion()}`, enabled: false })
      );
      about.submenu?.append(
        new MenuItem({
          label: 'A fork of goose v1.38 — a lighter version of goose',
          enabled: false,
        })
      );
      helpMenu.submenu.append(about);
    }
  }

  if (menu) {
    translateMenuLabels(menu.items);
    Menu.setApplicationMenu(menu);
  }
}
