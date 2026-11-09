// On-screen keyboard data. Keys are clicked, never typed, so any key can be chosen, including ones
// this PC's keyboard doesn't have. Each key is [label, token, width in key units].
const k = (label, token = label.toLowerCase(), width = 1) => [label, token, width];
const gap = (width) => [null, null, width];
const KEY_GAP = 10;
const MAX_KEY_GAP = 100;

const FUNCTION_ROWS = [
  [k('Esc', 'esc'), gap(1), ...[1, 2, 3, 4].map((n) => k(`F${n}`)), gap(.5), ...[5, 6, 7, 8].map((n) => k(`F${n}`)), gap(.5), ...[9, 10, 11, 12].map((n) => k(`F${n}`))],
  [gap(2), ...[13, 14, 15, 16].map((n) => k(`F${n}`)), gap(.5), ...[17, 18, 19, 20].map((n) => k(`F${n}`)), gap(.5), ...[21, 22, 23, 24].map((n) => k(`F${n}`))],
];

const bottomRow = [
  k('Ctrl', 'ctrl', 1.5), k('Win', 'win', 1.25), k('Alt', 'alt', 1.25), k('Space', 'space', 5.5),
  k('AltGr', 'altgr', 1.25), k('RWin', 'rwin', 1.25), k('RCtrl', 'rctrl', 1.5),
];

const span = (from, to) => Array.from({ length: to - from + 1 }, (_, i) => from + i);

/**
 * The typing keys by physical position (scan code), labelled with what they type on this PC's
 * layout, from `keyboard` in the app. ISO boards have a key beside left Shift; ABNT2 one more
 * beside right Shift.
 */
function typingRows(board) {
  const info = (scan) => board.byToken.get(board.scans[scan]);
  const key = (scan, width = 1) => {
    const found = info(scan);
    return found ? k(found.label ?? found.name, found.token, width) : gap(width);
  };
  const keys = (from, to) => span(from, to).map((scan) => key(scan));
  const iso = info(0x56)?.label && info(0x56).label !== info(0x2b)?.label;
  const abnt = !!info(0x73)?.label;
  return [
    [key(0x29), ...keys(0x02, 0x0d), k('Backspace', 'backspace', 2)],
    [k('Tab', 'tab', 1.5), ...keys(0x10, 0x1b), iso ? k('Enter', 'enter', 1.5) : key(0x2b, 1.5)],
    [k('Caps', 'vk14', 1.75), ...keys(0x1e, 0x28), ...(iso ? [key(0x2b), k('Enter', 'enter', 1.25)] : [k('Enter', 'enter', 2.25)])],
    [k('Shift', 'shift', iso ? 1.25 : 2.25), ...(iso ? [key(0x56)] : []), ...keys(0x2c, 0x35), ...(abnt ? [key(0x73)] : []), k('RShift', 'rshift', abnt ? 1.75 : 2.75)],
    bottomRow,
  ];
}

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

// Key events report Ctrl, Shift, Alt and Windows without their side, except in `code`.
const SIDED_KEYS = {
  ShiftLeft: 'shift', ShiftRight: 'rshift', ControlLeft: 'ctrl', ControlRight: 'rctrl',
  AltLeft: 'alt', AltRight: 'altgr', MetaLeft: 'win', MetaRight: 'rwin',
};

/** The hotkey token of a pressed key, or null for events that carry no key (IME, unidentified). */
function eventToken(e, board) {
  if (SIDED_KEYS[e.code]) return SIDED_KEYS[e.code];
  const code = e.keyCode;
  if (!code || code === 229 || code > 0xfe) return null;
  return board.byCode.get(code)?.token ?? `vk${code.toString(16).padStart(2, '0')}`;
}
