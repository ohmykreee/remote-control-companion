/**
 * All interface copy lives here, one object per language. Keep it user-facing:
 * say what a feature does, what the user can do, and what limits matter. Never
 * expose module names, protocols, runtimes or build details in these strings.
 */
export type AppLanguage = 'zh' | 'en';

const zh = {
  app: {
    name: 'DSH 远控伴侣',
    shortName: '远控伴侣',
    menuLabel: '应用',
    loading: '正在准备窗口',
    bootFailed: '窗口未能启动',
    initFailed: '窗口没能完成初始化，请关闭后重新打开。',
  },
  titlebar: {
    refresh: '刷新远控页面',
    refreshShort: '刷新',
    refreshing: '刷新中',
    minimize: '最小化',
    maximize: '最大化',
    close: '关闭',
    settings: '设置',
    about: '关于本软件',
    backToHome: '回到连接页',
    alwaysOnTop: '置顶窗口',
    cancelAlwaysOnTop: '取消置顶窗口',
    alwaysOnTopFailed: '无法切换窗口置顶状态，请重试。',
    quit: '退出',
  },
  language: {
    label: '语言',
    switchTo: '切换语言',
  },
  home: {
    eyebrow: 'DSH REMOTE CONTROL',
    connectTitle: '连接远控',
    title: '把远控台放进独立窗口',
    subtitle:
      '输入 dsh-remote-control 在电脑上显示的远控地址。连接后，页面会占据整个窗口，标题栏菜单仍随时可用。',
    addressLabel: '远控地址',
    addressPlaceholder: 'http://192.168.1.20:38765/',
    connect: '前往',
    connecting: '连接中',
    remember: '记住该地址',
    rememberHint: '下次打开时自动填入',
    privacy: '地址和设置只保存在本机程序目录的 Data 文件夹。',
    invalidAddress: '请输入完整的远控地址，例如 http://192.168.1.20:38765/。',
    connectFailed: '无法打开这个远控地址，请确认 dsh-remote-control 已启动且地址可访问。',
    emptyAddress: '请先输入远控地址。',
  },
  settings: {
    title: '设置',
    subtitle: '调整这个窗口的输入和显示方式。修改会自动保存。',
    close: '关闭设置',
    microphone: {
      title: '麦克风',
      description: '选择远控页面使用哪个输入设备。',
      label: '系统麦克风',
      defaultDevice: '系统默认麦克风',
      preview: '输入电平',
      listening: '正在预览',
      preparing: '准备中',
      idle: '选择麦克风后可预览输入',
      denied: '无法使用麦克风，请在 Windows 隐私设置中允许麦克风访问。',
      unavailable: '没有找到可用的麦克风。',
      fallbackOption: '已保存的麦克风（当前不可用）',
      fallback: '无法开始预览，请检查麦克风是否被其他程序占用。',
    },
    screenshot: {
      title: 'VR 截图',
      description: '让远控页面从本机的 VR 画面获取图片，替代从剪贴板读取图片。',
      replace: '替换剪切板为 VR 截图',
      replaceHint: '远控页面检测到本软件后，会优先使用这里的截图。',
      delay: '延迟 3 秒后截图',
      delayHint: '等待三秒，每秒响一声；最后一声咔嚓后截图。',
      note: '需要 SteamVR 正在运行。修改后重新载入远控页面生效。',
    },
    security: {
      title: '安全上下文',
      description: '部分远控功能要求页面运行在安全环境中。',
      force: '强制开启安全上下文',
      forceHint:
        '为输入的 http 地址启用安全环境，允许麦克风、语音输入和剪贴板读取。重新载入后生效。',
      note: '仅对当前输入的远控地址生效。使用 https 时无需开启。',
    },
    window: {
      title: '窗口',
      description: '调整窗口的显示、快捷键和靠边行为。',
      opacity: '窗口透明度',
      opacityHint: '最低 5%，避免窗口透明到找不到。',
      bossKey: '老板键',
      bossKeyHint: '点击后按下组合键，在任意窗口快速隐藏或显示本窗口。',
      bossKeyPlaceholder: '点击后按下组合键',
      bossKeyRecording: '请按下组合键…',
      bossKeyInvalid: '快捷键需包含 Ctrl、Alt 或 Shift。',
      bossKeyClear: '清除',
      edge: '窗口靠边隐藏',
      edgeHint: '窗口靠近选中的屏幕边缘后滑出屏幕，鼠标移到附近再滑回来。',
      edgeDirections: '靠边隐藏方向',
      edgeDirectionsHint: '可多选；未选择时不会隐藏。',
      edgeTop: '上',
      edgeLeft: '左',
      edgeRight: '右',
      reduceMotion: '减少动态效果',
      reduceMotionHint: '关闭窗口滑动、界面过渡和远控页面的切页动画。',
      edgeNote: '窗口最大化时不会靠边隐藏。',
    },
    autoSaved: '设置已保存',
    saveFailed: '设置未能保存，请重试。',
  },
  overlay: {
    menuHint: '远控页面已暂时隐藏，关闭菜单后继续显示。',
  },
  about: {
    title: '关于 DSH 远控伴侣',
    description:
      '一个专门显示 dsh-remote-control 远控页面的桌面窗口，让远控、麦克风和 VR 截图在电脑上顺手可用。',
    version: '版本',
    data: '数据目录',
    platform: '平台',
    close: '知道了',
  },
  remote: {
    reloading: '正在重新载入远控页面',
    refreshFailed: '刷新失败，请确认远控地址仍然可访问。',
    goHomeFailed: '无法返回连接页，请重试。',
    notConnected: '当前没有可刷新的远控页面。',
  },
  toast: {
    settingsSaved: '设置已保存',
  },
};

type CopySchema = typeof zh;

const en: CopySchema = {
  app: {
    name: 'DSH Remote Companion',
    shortName: 'Remote Companion',
    menuLabel: 'App',
    loading: 'Preparing window',
    bootFailed: 'Window failed to start',
    initFailed: 'The window could not finish starting. Close and reopen it.',
  },
  titlebar: {
    refresh: 'Refresh remote page',
    refreshShort: 'Refresh',
    refreshing: 'Refreshing',
    minimize: 'Minimize',
    maximize: 'Maximize',
    close: 'Close',
    settings: 'Settings',
    about: 'About',
    backToHome: 'Back to connection page',
    alwaysOnTop: 'Always on top',
    cancelAlwaysOnTop: 'Stop always on top',
    alwaysOnTopFailed: 'Could not change the always-on-top state. Try again.',
    quit: 'Quit',
  },
  language: {
    label: 'Language',
    switchTo: 'Switch language',
  },
  home: {
    eyebrow: 'DSH REMOTE CONTROL',
    connectTitle: 'Connect to remote',
    title: 'Put the remote console in its own window',
    subtitle:
      'Enter the remote address shown by dsh-remote-control. After connecting, the page fills the window and the titlebar menu stays available.',
    addressLabel: 'Remote address',
    addressPlaceholder: 'http://192.168.1.20:38765/',
    connect: 'Connect',
    connecting: 'Connecting',
    remember: 'Remember this address',
    rememberHint: 'Prefilled next time',
    privacy: 'The address and settings are stored only in the Data folder next to this program.',
    invalidAddress: 'Enter a complete remote address, for example http://192.168.1.20:38765/.',
    connectFailed:
      'Could not open this remote address. Check that dsh-remote-control is running and reachable.',
    emptyAddress: 'Enter a remote address first.',
  },
  settings: {
    title: 'Settings',
    subtitle: 'Adjust input and display for this window. Changes save automatically.',
    close: 'Close settings',
    microphone: {
      title: 'Microphone',
      description: 'Choose which input device the remote page uses.',
      label: 'System microphone',
      defaultDevice: 'System default microphone',
      preview: 'Input level',
      listening: 'Previewing',
      preparing: 'Preparing',
      idle: 'Select a microphone to preview input',
      denied:
        'Microphone access is unavailable. Allow microphone access in Windows privacy settings.',
      unavailable: 'No microphone was found.',
      fallbackOption: 'Saved microphone (currently unavailable)',
      fallback: 'Could not start the preview. Check whether another app is using the microphone.',
    },
    screenshot: {
      title: 'VR screenshots',
      description: 'Let the remote page use the local VR view instead of a clipboard image.',
      replace: 'Replace clipboard with VR screenshot',
      replaceHint: 'The remote page prefers this screenshot when it detects the companion.',
      delay: 'Delay screenshot by 3 seconds',
      delayHint:
        'Wait three seconds with one beep per second; capture after the final shutter sound.',
      note: 'SteamVR must be running. Reload the remote page after changing this.',
    },
    security: {
      title: 'Secure context',
      description: 'Some remote features require the page to run in a secure context.',
      force: 'Force secure context',
      forceHint:
        'Enable a secure context for the entered http address so microphone, voice input, and clipboard reads work. Reload to apply.',
      note: 'Applies only to the current remote address. Not needed for https.',
    },
    window: {
      title: 'Window',
      description: 'Adjust display, shortcuts, and edge behavior for this window.',
      opacity: 'Window opacity',
      opacityHint: 'Minimum 5%, so the window cannot become too transparent to find.',
      bossKey: 'Boss key',
      bossKeyHint:
        'Click, then press a key combination to hide or show this window from anywhere.',
      bossKeyPlaceholder: 'Click, then press a shortcut',
      bossKeyRecording: 'Press a shortcut…',
      bossKeyInvalid: 'The shortcut must include Ctrl, Alt, or Shift.',
      bossKeyClear: 'Clear',
      edge: 'Hide at screen edge',
      edgeHint:
        'When the window is near a selected screen edge it slides away; move the pointer nearby to bring it back.',
      edgeDirections: 'Edge directions',
      edgeDirectionsHint: 'Select any combination; no directions means it will not hide.',
      edgeTop: 'Top',
      edgeLeft: 'Left',
      edgeRight: 'Right',
      reduceMotion: 'Reduce motion',
      reduceMotionHint:
        'Turn off window sliding, UI transitions, and remote page transition animations.',
      edgeNote: 'Edge hiding is disabled while the window is maximized.',
    },
    autoSaved: 'Settings saved',
    saveFailed: 'Settings could not be saved. Try again.',
  },
  overlay: {
    menuHint: 'The remote page is temporarily hidden. It will return when the menu closes.',
  },
  about: {
    title: 'About DSH Remote Companion',
    description:
      'A desktop window for the dsh-remote-control page, so remote control, the microphone, and VR screenshots are easy to use on your computer.',
    version: 'Version',
    data: 'Data folder',
    platform: 'Platform',
    close: 'Done',
  },
  remote: {
    reloading: 'Reloading remote page',
    refreshFailed: 'Refresh failed. Check that the remote address is still reachable.',
    goHomeFailed: 'Could not return to the connection page. Try again.',
    notConnected: 'There is no remote page to refresh.',
  },
  toast: {
    settingsSaved: 'Settings saved',
  },
};

const dictionaries: Record<AppLanguage, CopySchema> = { zh, en };
let active = $state<AppLanguage>('zh');

export function setCopyLocale(value: string): void {
  active = value === 'en' ? 'en' : 'zh';
}

export function currentCopyLocale(): AppLanguage {
  return active;
}

export const copy = new Proxy({} as CopySchema, {
  get(_target, key) {
    return dictionaries[active][key as keyof CopySchema];
  },
});
