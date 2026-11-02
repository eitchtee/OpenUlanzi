// On-screen keyboard data. Keys are clicked, never typed, so any key can be chosen, including ones
// this PC's keyboard doesn't have. Each key is [label, token, width in key units].
const k = (label, token = label.toLowerCase(), width = 1) => [label, token, width];
const row = (text) => [...text].map((c) => k(c));
const gap = (width) => [null, null, width];

const FUNCTION_ROWS = [
  [k('Esc', 'esc'), gap(1), ...[1, 2, 3, 4].map((n) => k(`F${n}`)), gap(.5), ...[5, 6, 7, 8].map((n) => k(`F${n}`)), gap(.5), ...[9, 10, 11, 12].map((n) => k(`F${n}`))],
  [gap(2), ...[13, 14, 15, 16].map((n) => k(`F${n}`)), gap(.5), ...[17, 18, 19, 20].map((n) => k(`F${n}`)), gap(.5), ...[21, 22, 23, 24].map((n) => k(`F${n}`))],
];

const bottomRow = [
  k('Ctrl', 'ctrl', 1.5), k('Win', 'win', 1.25), k('Alt', 'alt', 1.25), k('Space', 'space', 5.5),
  k('AltGr', 'altgr', 1.25), k('RWin', 'rwin', 1.25), k('RCtrl', 'rctrl', 1.5),
];

const LAYOUTS = {
  'US (ANSI)': [
    [k('`', 'vkc0'), ...row('1234567890'), k('-', 'vkbd'), k('=', 'vkbb'), k('Backspace', 'backspace', 2)],
    [k('Tab', 'tab', 1.5), ...row('QWERTYUIOP'), k('[', 'vkdb'), k(']', 'vkdd'), k('\\', 'vkdc', 1.5)],
    [k('Caps', 'vk14', 1.75), ...row('ASDFGHJKL'), k(';', 'vkba'), k("'", 'vkde'), k('Enter', 'enter', 2.25)],
    [k('Shift', 'shift', 2.25), ...row('ZXCVBNM'), k(',', 'vkbc'), k('.', 'vkbe'), k('/', 'vkbf'), k('RShift', 'rshift', 2.75)],
    bottomRow,
  ],
  'Brazilian (ABNT2)': [
    [k("'", 'vkc0'), ...row('1234567890'), k('-', 'vkbd'), k('=', 'vkbb'), k('Backspace', 'backspace', 2)],
    [k('Tab', 'tab', 1.5), ...row('QWERTYUIOP'), k('´', 'vkdb'), k('[', 'vkdd'), k('Enter', 'enter', 1.5)],
    [k('Caps', 'vk14', 1.75), ...row('ASDFGHJKL'), k('Ç', 'vkba'), k('~', 'vkde'), k(']', 'vkdc'), gap(1.25)],
    [k('Shift', 'shift', 1.25), k('\\', 'vke2'), ...row('ZXCVBNM'), k(',', 'vkbc'), k('.', 'vkbe'), k(';', 'vkbf'), k('/', 'vkc1'), k('RShift', 'rshift', 1.75)],
    bottomRow,
  ],
};

const NAV_BLOCK = [
  [k('PrtSc', 'printscreen'), k('Ins', 'insert'), k('Home', 'home'), k('PgUp', 'pageup')],
  [k('Del', 'delete'), k('End', 'end'), k('PgDn', 'pagedown'), gap(1)],
  [gap(1), k('Up', 'up'), gap(2)],
  [k('Left', 'left'), k('Down', 'down'), k('Right', 'right'), gap(1)],
];

const NUMPAD = [
  [k('Num', 'vk90'), k('/', 'vk6f'), k('*', 'vk6a'), k('-', 'vk6d')],
  [k('7', 'vk67'), k('8', 'vk68'), k('9', 'vk69'), k('+', 'vk6b')],
  [k('4', 'vk64'), k('5', 'vk65'), k('6', 'vk66'), k('.', 'vk6e')],
  [k('1', 'vk61'), k('2', 'vk62'), k('3', 'vk63'), k('0', 'vk60')],
];

const MEDIA_KEYS = [
  k('Mute', 'mute', 2), k('Volume down', 'volume_down', 2), k('Volume up', 'volume_up', 2),
  k('Play or pause', 'play_pause', 2), k('Previous', 'prev_track', 2), k('Next', 'next_track', 2), k('Stop', 'media_stop', 2),
];
