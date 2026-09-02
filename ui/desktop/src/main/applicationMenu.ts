// Owns augmentation, localization, and installation of the full Desktop application menu.
// Extracted from ui/desktop/src/main.ts in a behavior-preserving modularization.
// The compatibility facade imports installApplicationMenu; it re-exports none.

import type { App, BrowserWindow as BrowserWindowType, MenuItemConstructorOptions } from 'electron';
import { BrowserWindow, Menu, MenuItem } from 'electron';
import { rendererEventChannels } from '../ipc/channels';
import type { Settings } from '../utils/settings';
import { getKeyboardShortcuts } from '../utils/settings';

export interface ApplicationMenuDependencies {
  app: App;
  settings: Settings;
  bundledVersion?: string;
  menuT: (label: string) => string;
  translateMenuLabels: (items: MenuItem[]) => void;
  createNewWindow: () => Promise<BrowserWindowType | undefined>;
  openDirectoryDialog: () => Promise<unknown>;
  buildRecentFilesMenu: () => MenuItemConstructorOptions[];
  focusWindow: () => void;
  createLauncher: () => BrowserWindowType;
}

export function installApplicationMenu(dependencies: ApplicationMenuDependencies): void {
  const {
    app,
    settings,
    bundledVersion,
    menuT,
    translateMenuLabels,
    createNewWindow,
    openDirectoryDialog,
    buildRecentFilesMenu,
    focusWindow,
    createLauncher,
  } = dependencies;
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
            const focusedWindow = BrowserWindow.getFocusedWindow();
            if (focusedWindow) {
              focusedWindow.webContents.send(rendererEventChannels.setView, 'settings');
            }
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
        click() {
          const focusedWindow = BrowserWindow.getFocusedWindow();
          if (focusedWindow) {
            focusedWindow.webContents.send(rendererEventChannels.findCommand);
          }
        },
      },
      {
        label: menuT('Find Next'),
        accelerator: shortcuts.findNext || undefined,
        click() {
          const focusedWindow = BrowserWindow.getFocusedWindow();
          if (focusedWindow) {
            focusedWindow.webContents.send(rendererEventChannels.findNext);
          }
        },
      },
      {
        label: menuT('Find Previous'),
        accelerator: shortcuts.findPrevious || undefined,
        click() {
          const focusedWindow = BrowserWindow.getFocusedWindow();
          if (focusedWindow) {
            focusedWindow.webContents.send(rendererEventChannels.findPrevious);
          }
        },
      },
      {
        label: menuT('Use Selection for Find'),
        accelerator: process.platform === 'darwin' ? 'Command+E' : undefined,
        click() {
          const focusedWindow = BrowserWindow.getFocusedWindow();
          if (focusedWindow) {
            focusedWindow.webContents.send(rendererEventChannels.useSelectionFind);
          }
        },
        visible: process.platform === 'darwin', // Only show on Mac
      },
    ]);

    editMenu.submenu.insert(
      selectAllIndex + 1,
      new MenuItem({
        label: menuT('Find'),
        submenu: findSubmenu,
      })
    );
  }

  const fileMenu = menu?.items.find((item) => item.label === 'File');

  if (fileMenu?.submenu) {
    // Use a counter to track the actual insertion index
    let menuIndex = 0;

    if (shortcuts.newChat) {
      fileMenu.submenu.insert(
        menuIndex++,
        new MenuItem({
          label: menuT('New Chat'),
          accelerator: shortcuts.newChat,
          click() {
            const focusedWindow = BrowserWindow.getFocusedWindow();
            if (focusedWindow) {
              focusedWindow.webContents.send(rendererEventChannels.newChat);
            }
          },
        })
      );
    }

    if (shortcuts.newChatWindow) {
      fileMenu.submenu.insert(
        menuIndex++,
        new MenuItem({
          label: menuT('New Chat Window'),
          accelerator: shortcuts.newChatWindow,
          click() {
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
        new MenuItem({
          label: menuT('Recent Directories'),
          submenu: recentFilesSubmenu,
        })
      );
    }

    fileMenu.submenu.insert(menuIndex++, new MenuItem({ type: 'separator' }));

    if (shortcuts.focusWindow) {
      fileMenu.submenu.append(
        new MenuItem({
          label: menuT('Focus Gosling Window'),
          accelerator: shortcuts.focusWindow,
          click() {
            focusWindow();
          },
        })
      );
    }

    if (shortcuts.quickLauncher) {
      fileMenu.submenu.append(
        new MenuItem({
          label: menuT('Quick Launcher'),
          accelerator: shortcuts.quickLauncher,
          click() {
            createLauncher();
          },
        })
      );
    }
  }

  if (menu) {
    let windowMenu = menu.items.find((item) => item.label === 'Window');

    if (!windowMenu) {
      windowMenu = new MenuItem({
        label: menuT('Window'),
        submenu: Menu.buildFromTemplate([]),
      });

      const helpMenuIndex = menu.items.findIndex((item) => item.label === 'Help');
      if (helpMenuIndex >= 0) {
        menu.items.splice(helpMenuIndex, 0, windowMenu);
      } else {
        menu.items.push(windowMenu);
      }
    }

    if (windowMenu.submenu) {
      if (shortcuts.alwaysOnTop) {
        windowMenu.submenu.append(
          new MenuItem({
            label: menuT('Always on Top'),
            type: 'checkbox',
            accelerator: shortcuts.alwaysOnTop,
            click(menuItem) {
              const focusedWindow = BrowserWindow.getFocusedWindow();
              if (focusedWindow) {
                const isAlwaysOnTop = menuItem.checked;

                if (process.platform === 'darwin') {
                  focusedWindow.setAlwaysOnTop(isAlwaysOnTop, 'floating');
                } else {
                  focusedWindow.setAlwaysOnTop(isAlwaysOnTop);
                }

                console.log(
                  `[Main] Set always-on-top to ${isAlwaysOnTop} for window ${focusedWindow.id}`
                );
              }
            },
          })
        );
      }
    }

    const viewMenu = menu.items.find((item) => item.label === 'View');
    if (viewMenu?.submenu && shortcuts.toggleNavigation) {
      viewMenu.submenu.append(new MenuItem({ type: 'separator' }));
      viewMenu.submenu.append(
        new MenuItem({
          label: menuT('Toggle Navigation'),
          accelerator: shortcuts.toggleNavigation,
          click() {
            const focusedWindow = BrowserWindow.getFocusedWindow();
            if (focusedWindow) {
              focusedWindow.webContents.send(rendererEventChannels.toggleNavigation);
            }
          },
        })
      );
    }
  }

  // on macOS, the topbar is hidden
  if (menu && process.platform !== 'darwin') {
    let helpMenu = menu.items.find((item) => item.label === 'Help');

    // If Help menu doesn't exist, create it and add it to the menu
    if (!helpMenu) {
      helpMenu = new MenuItem({
        label: menuT('Help'),
        submenu: Menu.buildFromTemplate([]), // Start with an empty submenu
      });
      // Find a reasonable place to insert the Help menu, usually near the end
      const insertIndex = menu.items.length > 0 ? menu.items.length - 1 : 0;
      menu.items.splice(insertIndex, 0, helpMenu);
    }

    // Ensure the Help menu has a submenu before appending
    if (helpMenu.submenu) {
      // Add a separator before the About item if the submenu is not empty
      if (helpMenu.submenu.items.length > 0) {
        helpMenu.submenu.append(new MenuItem({ type: 'separator' }));
      }

      // Create the About Gosling menu item with a submenu
      const aboutGoslingMenuItem = new MenuItem({
        label: menuT('About Gosling'),
        submenu: Menu.buildFromTemplate([]), // Start with an empty submenu for About
      });

      // Add the version and provenance items (display only) to the About Gosling submenu
      if (aboutGoslingMenuItem.submenu) {
        aboutGoslingMenuItem.submenu.append(
          new MenuItem({
            label: `Gosling v${bundledVersion || app.getVersion()}`,
            enabled: false,
          })
        );
        aboutGoslingMenuItem.submenu.append(
          new MenuItem({
            label: 'A fork of goose v1.38 — a lighter version of goose',
            enabled: false,
          })
        );
      }

      helpMenu.submenu.append(aboutGoslingMenuItem);
    }
  }

  if (menu) {
    // Translate labels (including Electron's default top-level entries
    // File/Edit/View/Window/Help and submenu items populated by roles) before
    // installing the menu. Called last so the lookups above that match on the
    // English labels still succeed.
    translateMenuLabels(menu.items);
    Menu.setApplicationMenu(menu);
  }
}
