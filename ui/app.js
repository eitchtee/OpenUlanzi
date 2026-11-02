const KEYS = [...Array(13).keys()].map(String);
const KNOBS = ['knob1', 'knob2', 'knob3'];
const SCREENS = [...KEYS, 'wide'];
const CONTROL_NAMES = { wide: 'Wide screen', side1: 'Left side button', side2: 'Right side button', knob1: 'Left knob', knob2: 'Middle knob', knob3: 'Right knob' };
const APP_LAYER = '@app';
const DEFAULT_PROFILE = '@default';
const BACK_KEY = '0';
const BACK_BINDING = { label: 'Back', icon: 'ph:arrow-left-bold' };
const WIDE_DISPLAYS = [
  ['image', 'Label and icon'], ['page_indicator', 'Page number'], ['dial', 'Analog clock'], ['time', 'Time'],
  ['time_weekday', 'Time and weekday'], ['time_date', 'Time and date'], ['date_time_weekday', 'Date, time and weekday'],
  ['stats', 'CPU and memory'],
];

/* Every entry can be dragged onto a control, or clicked to apply it to the selected one. */
const ACTION_GROUPS = [
  ['Pages', [
    { name: 'Open page', icon: 'folder-open-bold', apply: (b, ctx) => { b.action = { page: ctx.otherPage() }; } },
    { name: 'Open folder', icon: 'folder-bold', folder: true, apply: (b, ctx) => { b.action = { page: ctx.folder() }; b.icon ||= 'ph:folder-bold'; } },
    { name: 'Go back', icon: 'arrow-u-up-left-bold', apply: (b) => { b.action = 'back'; b.label ||= 'Back'; b.icon ||= 'ph:arrow-u-up-left-bold'; } },
    { name: 'Next page', icon: 'caret-right-bold', apply: (b) => { b.action = 'next_page'; b.icon ||= 'ph:caret-right-bold'; } },
    { name: 'Previous page', icon: 'caret-left-bold', apply: (b) => { b.action = 'prev_page'; b.icon ||= 'ph:caret-left-bold'; } },
    { name: 'Switch profile', icon: 'user-switch-bold', apply: (b) => { b.action = { profile: state.cfg.profile[0].name }; } },
    { name: 'Go to focused app profile', icon: 'app-window-bold', apply: (b) => { b.action = { profile: APP_LAYER }; b.label ||= 'App'; } },
    { name: 'Go to default profile', icon: 'house-bold', apply: (b) => { b.action = { profile: DEFAULT_PROFILE }; b.label ||= 'Home'; b.icon ||= 'ph:house-bold'; } },
    { name: 'Page number', icon: 'hash-bold', screen: true, apply: (b) => { b.display = 'page_indicator'; } },
  ]],
  ['Hold to show', [
    { name: 'Hold for a page', icon: 'hand-grabbing-bold', hold: true, apply: (b, ctx) => { b.hold = { show: ctx.otherPage() }; } },
    { name: 'Hold for the focused app', icon: 'app-window-bold', hold: true, apply: (b) => { b.hold = { show: APP_LAYER }; b.label ||= 'App'; } },
    { name: 'Hold to run an action', icon: 'timer-bold', hold: true, apply: (b) => { b.hold = { hotkey: '' }; } },
  ]],
  ['System', [
    { name: 'Hotkey', icon: 'keyboard-bold', apply: (b) => { b.action = { hotkey: '' }; } },
    { name: 'Type text', icon: 'text-t-bold', apply: (b) => { b.action = { text: '' }; } },
    { name: 'Open app or file', icon: 'rocket-launch-bold', apply: (b) => { b.action = { open: '' }; } },
    { name: 'Close app', icon: 'x-square-bold', apply: (b) => { b.action = { close: '' }; } },
    { name: 'Website', icon: 'globe-bold', apply: (b) => { b.action = { open: 'https://' }; b.icon ||= 'ph:globe-bold'; } },
    { name: 'Scroll up', icon: 'arrow-line-up-bold', apply: (b) => { b.action = { scroll_up: 3 }; } },
    { name: 'Scroll down', icon: 'arrow-line-down-bold', apply: (b) => { b.action = { scroll_down: 3 }; } },
    { name: 'Scroll left', icon: 'arrow-line-left-bold', apply: (b) => { b.action = { scroll_left: 3 }; } },
    { name: 'Scroll right', icon: 'arrow-line-right-bold', apply: (b) => { b.action = { scroll_right: 3 }; } },
    { name: 'Deck brightness', icon: 'sun-bold', apply: (b) => { b.action = { brightness: 10 }; } },
  ]],
  ['Media', [
    { name: 'Play or pause', icon: 'play-pause-bold', apply: (b) => media(b, 'play_pause', 'Play', 'play-pause-fill') },
    { name: 'Next track', icon: 'skip-forward-bold', apply: (b) => media(b, 'next_track', 'Next', 'skip-forward-fill') },
    { name: 'Previous track', icon: 'skip-back-bold', apply: (b) => media(b, 'prev_track', 'Previous', 'skip-back-fill') },
    { name: 'Mute', icon: 'speaker-x-bold', apply: (b) => media(b, 'mute', 'Mute', 'speaker-x-fill') },
    { name: 'Volume up', icon: 'speaker-high-bold', apply: (b) => media(b, 'volume_up', 'Louder', 'speaker-high-fill') },
    { name: 'Volume down', icon: 'speaker-low-bold', apply: (b) => media(b, 'volume_down', 'Quieter', 'speaker-low-fill') },
  ]],
  ['Focused app volume', [
    { name: 'App volume up', icon: 'speaker-simple-high-bold', apply: (b) => { b.action = { app_volume: 5 }; } },
    { name: 'App volume down', icon: 'speaker-simple-low-bold', apply: (b) => { b.action = { app_volume: -5 }; } },
    { name: 'Reset app volume', icon: 'arrow-counter-clockwise-bold', apply: (b) => { b.action = 'app_volume_reset'; } },
    { name: 'App volume knob', icon: 'speaker-simple-high-bold', knob: true, apply: (b) => { b.left = { app_volume: -5 }; b.right = { app_volume: 5 }; b.action = 'app_volume_reset'; b.label ||= 'App'; } },
  ]],
  ['Knobs', [
    { name: 'Volume knob', icon: 'speaker-high-bold', knob: true, apply: (b) => { b.left = { hotkey: 'volume_down' }; b.right = { hotkey: 'volume_up' }; b.action = { hotkey: 'mute' }; b.label ||= 'Volume'; } },
    { name: 'Vertical scroll knob', icon: 'arrows-down-up-bold', knob: true, apply: (b) => { b.left = { scroll_up: 3 }; b.right = { scroll_down: 3 }; b.label ||= 'Scroll'; } },
    { name: 'Horizontal scroll knob', icon: 'arrows-left-right-bold', knob: true, apply: (b) => { b.left = { scroll_left: 3 }; b.right = { scroll_right: 3 }; b.label ||= 'Scroll'; } },
    { name: 'Brightness knob', icon: 'sun-bold', knob: true, apply: (b) => { b.left = { brightness: -10 }; b.right = { brightness: 10 }; b.label ||= 'Brightness'; } },
  ]],
  ['Several actions', [
    { name: 'Sequence', icon: 'stack-bold', apply: (b) => { b.action = { sequence: [] }; } },
    { name: 'Toggle between actions', icon: 'swap-bold', apply: (b) => { b.action = { toggle: [] }; } },
    { name: 'Random action', icon: 'shuffle-bold', apply: (b) => { b.action = { random: [] }; } },
    { name: 'Wait', icon: 'hourglass-bold', wait: true, apply: (b) => { b.action = { delay: 200 }; } },
  ]],
  ['Sound', [
    { name: 'Play sound file', icon: 'music-note-bold', apply: (b) => { b.action = { play_sound: '' }; } },
    { name: 'Stop sound', icon: 'stop-bold', apply: (b) => { b.action = 'stop_sound'; } },
  ]],
];
const PALETTE = ACTION_GROUPS.flatMap(([, items]) => items);

function media(b, hotkey, label, icon) {
  b.action = { hotkey };
  b.label ||= label;
  b.icon ||= `ph:${icon}`;
}

const state = { device: '', devices: [], cfg: null, profile: 0, page: 0, selected: null, view: 'keys', status: null, icons: new Map(), collapsed: new Set(), clipboard: null, armed: null };
const $ = (id) => document.getElementById(id);

function remember(key, value) {
  try { localStorage.setItem(key, value); } catch { /* preferences are optional */ }
}
function recall(key) {
  try { return localStorage.getItem(key); } catch { return null; }
}

/** Tiny DOM builder: h('div', { class: 'x', onclick: f }, child, 'text'). */
function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k in el && k !== 'list') el[k] = v;
    else el.setAttribute(k, v);
  }
  el.append(...children.filter((c) => c !== null && c !== undefined));
  return el;
}

/** Inline SVG from the embedded set. The markup comes from our own binary, not from user data. */
function icon(name) {
  const span = h('span', { class: 'glyph', 'aria-hidden': 'true' });
  const inner = state.icons.get(name);
  if (inner) span.innerHTML = `<svg viewBox="0 0 256 256" fill="currentColor">${inner}</svg>`;
  return span;
}

const field = (label, control, hint) => h('label', { class: 'field' }, h('span', {}, label), control, hint ? h('p', { class: 'hint' }, hint) : null);
const profile = () => state.cfg.profile[state.profile];
const page = () => profile().page[state.page];
const isBackKey = (control) => !!page().folder && control === BACK_KEY;
const binding = (control) => page().keys[control];
const controlName = (c) => CONTROL_NAMES[c] ?? `Key ${Number(c) + 1}`;
const isKnob = (c) => KNOBS.includes(c);
const hasScreen = (c) => SCREENS.includes(c);
const hasDials = () => state.cfg.model === 'd200x';

function notify(text, bad = false) {
  const el = $('notice');
  el.textContent = text;
  el.className = bad ? 'notice bad' : 'notice';
  el.hidden = false;
  clearTimeout(notify.timer);
  if (!bad) notify.timer = setTimeout(() => { el.hidden = true; }, 2600);
}

let saveTimer = null;

async function flushSave() {
  if (saveTimer === null) return;
  clearTimeout(saveTimer);
  saveTimer = null;
  try {
    await invoke('save_config', { device: state.device, config: state.cfg });
    $('notice').hidden = true;
  } catch (e) {
    notify(`Not saved: ${e}`, true);
  }
}

function save() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(flushSave, 250);
}

/** Change a control's binding, creating it if needed and dropping it if nothing is left. */
function edit(control, change, persist = true) {
  const b = page().keys[control] ?? {};
  change(b);
  for (const k of Object.keys(b)) if (b[k] === '' || b[k] === null || b[k] === undefined) delete b[k];
  if (Object.keys(b).length) page().keys[control] = b;
  else delete page().keys[control];
  if (persist) save();
  drawDeck();
}

const context = {
  otherPage() {
    const other = profile().page.find((p, i) => i !== state.page);
    return other ? other.name : page().name;
  },
  folder() {
    return profile().page.find((p) => p.folder)?.name ?? page().name;
  },
};

function wideDisplay(b) {
  if (b?.display) return b.display;
  return b?.label || b?.icon || b?.color || b?.background ? 'image' : state.cfg.wide;
}

function visualOf(b, control) {
  const display = control === 'wide' ? wideDisplay(b) : (b?.display ?? 'image');
  return {
    label: b?.label ?? '',
    icon: b?.icon ?? '',
    color: b?.color ?? state.cfg.color,
    background: b?.background ?? state.cfg.background,
    display,
    big: display === 'page_indicator' ? '1/2' : '',
  };
}

// ---------------------------------------------------------------- keys view
function focusControl(control) {
  document.querySelector(`[data-control="${control}"]`)?.focus();
}

function select(control) {
  state.selected = control;
  drawDeck();
  drawInspector();
  if (control !== null) focusControl(control);
}

function clearControl(control) {
  delete page().keys[control];
  save();
  drawDeck();
  drawInspector();
  focusControl(control);
}

function pasteControl(control) {
  const { binding: copied, knob: fromKnob } = state.clipboard;
  if (fromKnob !== isKnob(control)) return notify('Knob settings only paste onto knobs, and key settings onto keys.', true);
  page().keys[control] = structuredClone(copied);
  save();
  drawDeck();
  drawInspector();
  focusControl(control);
}

function closeMenu(restoreFocus = false) {
  const menu = document.getElementById('menu');
  if (!menu) return;
  menu.remove();
  if (restoreFocus) state.menuReturn?.focus?.();
}

/** Where a menu opens: the pointer, or the element itself when opened from the keyboard. */
function menuPoint(event) {
  if (event.clientX || event.clientY) return { x: event.clientX, y: event.clientY };
  const box = event.currentTarget.getBoundingClientRect();
  return { x: box.left, y: box.bottom };
}

function openMenu(at, items) {
  closeMenu();
  state.menuReturn = document.activeElement;
  const entries = items.filter(Boolean).map((item) => (item === '-' ? h('hr') : h('button', {
    role: 'menuitem',
    class: item.danger ? 'danger' : '',
    disabled: !!item.disabled,
    onclick: () => { closeMenu(); item.run(); },
  }, item.icon ? icon(item.icon) : null, item.label, item.checked ? h('span', { class: 'tick' }, icon('check-bold')) : null)));
  const menu = h('div', { id: 'menu', class: 'menu', role: 'menu' }, ...entries);
  menu.addEventListener('keydown', (e) => {
    const buttons = [...menu.querySelectorAll('button:not(:disabled)')];
    const step = { ArrowDown: 1, ArrowUp: -1 }[e.key];
    if (!step || !buttons.length) return;
    e.preventDefault();
    const at = buttons.indexOf(document.activeElement);
    buttons[(at + step + buttons.length) % buttons.length].focus();
  });
  document.body.append(menu);
  // Flip at the window edges, and grow from whichever corner sits at the pointer.
  const flipX = at.x + menu.offsetWidth + 4 > innerWidth;
  const flipY = at.y + menu.offsetHeight + 4 > innerHeight;
  menu.style.left = `${flipX ? at.x - menu.offsetWidth : at.x}px`;
  menu.style.top = `${flipY ? Math.max(4, at.y - menu.offsetHeight) : at.y}px`;
  menu.style.transformOrigin = `${flipX ? 'right' : 'left'} ${flipY ? 'bottom' : 'top'}`;
  menu.querySelector('button:not(:disabled)')?.focus({ preventScroll: true });
}

function controlMenu(at, control) {
  const b = binding(control);
  openMenu(at, [
    { label: 'Copy', icon: 'copy-bold', disabled: !b, run: () => { state.clipboard = { binding: structuredClone(b), knob: isKnob(control) }; } },
    { label: 'Paste', icon: 'clipboard-text-bold', disabled: !state.clipboard, run: () => pasteControl(control) },
    '-',
    { label: 'Clear', icon: 'eraser-bold', danger: true, disabled: !b, run: () => clearControl(control) },
  ]);
}

function iconButton(glyph, label, run) {
  return h('button', { type: 'button', class: 'icon-button', title: label, 'aria-label': label, onclick: run }, icon(glyph));
}

function confirmDialog(title, text, action, run) {
  openDialog(h('h2', {}, title), h('p', {}, text), h('div', { class: 'row done' },
    h('button', { class: 'plain', onclick: () => $('dialog').close() }, 'Cancel'),
    h('button', { class: 'plain danger', onclick: () => { $('dialog').close(); run(); } }, action)));
  $('dialog').classList.add('narrow');
}

function wireDrop(el, control) {
  el.addEventListener('dragover', (e) => { e.preventDefault(); el.classList.add('over'); });
  el.addEventListener('dragleave', () => el.classList.remove('over'));
  el.addEventListener('drop', (e) => {
    e.preventDefault();
    el.classList.remove('over');
    drop(e.dataTransfer.getData('text/plain'), control);
  });
}

function wireDrag(el, control) {
  el.draggable = true;
  el.dataset.control = control;
  el.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    e.stopPropagation();
    controlMenu(menuPoint(e), control);
  });
  el.addEventListener('keydown', (e) => {
    if (e.key === 'Delete') {
      e.preventDefault();
      clearControl(control);
    }
  });
  el.addEventListener('dragstart', (e) => {
    e.dataTransfer.setData('text/plain', `key:${control}`);
    el.classList.add('dragging');
  });
  el.addEventListener('dragend', () => el.classList.remove('dragging'));
  wireDrop(el, control);
}

function drop(data, target) {
  const [kind, value] = data.split(':');
  if (kind === 'palette') return applyPalette(PALETTE[Number(value)], target);
  if (kind !== 'key' || value === target || isBackKey(target)) return;
  if (isKnob(value) !== isKnob(target)) return notify('Knobs can only swap with knobs.', true);
  const keys = page().keys;
  const [from, to] = [keys[value], keys[target]];
  delete keys[value];
  delete keys[target];
  if (from) keys[target] = from;
  if (to) keys[value] = to;
  state.selected = target;
  save();
  draw();
}

/** Folder actions need a folder to point at, so the first one creates it. */
function ensureFolder(item) {
  if (item.folder && !profile().page.some((p) => p.folder)) {
    profile().page.push(newFolder());
    drawProfiles();
  }
}

function applyPalette(item, target) {
  ensureFolder(item);
  if (state.armed) return state.armed(item);
  if (item.wait) return notify('Wait only works inside a sequence.', true);
  if (target !== null && isBackKey(target)) return notify('A folder always keeps Back on this key.', true);
  if (target === null) return notify('Select a control first, or drag this onto one.');
  if (item.hold && isKnob(target)) return notify('Knobs cannot be held, because they only report a click.', true);
  if (item.knob && !isKnob(target)) return notify(`${item.name} goes on a knob.`, true);
  if (item.screen && !hasScreen(target)) return notify(`${item.name} needs a screen.`, true);
  edit(target, (b) => item.apply(b, context));
  select(target);
}

function keyButton(control, shown) {
  const locked = isBackKey(control);
  const b = locked ? BACK_BINDING : binding(control);
  const classes = ['ctl', 'key'];
  if (control === 'wide') classes.push('wide');
  if (b?.hold) classes.push('bound-layer');
  const display = visualOf(b, control).display;
  const firmware = !['image', 'page_indicator'].includes(display);
  if (firmware) classes.push('firmware');
  const el = h('button', {
    class: classes.join(' '),
    'aria-pressed': String(state.selected === control),
    'aria-label': `${controlName(control)}${b?.label ? `: ${b.label}` : ''}`,
    onclick: () => (locked ? notify('A folder always keeps Back on this key.') : select(control)),
  });
  if (firmware) el.dataset.face = WIDE_DISPLAYS.find(([id]) => id === display)?.[1] ?? '';
  else {
    // Keep showing the old image until the new one can paint, or the key flashes black.
    if (shown) el.append(shown);
    const visual = JSON.stringify(visualOf(b, control));
    el.dataset.visual = visual;
    keyImage(visualOf(b, control), control === 'wide').then(async (url) => {
      const img = h('img', { src: url, alt: '', draggable: false });
      await img.decode().catch(() => {});
      if (el.dataset.visual === visual) el.replaceChildren(img);
    });
  }
  if (locked) {
    el.classList.add('locked');
    el.dataset.control = control;
  } else {
    wireDrag(el, control);
  }
  return el;
}

function sideButton(control) {
  const b = binding(control);
  const el = h('button', {
    class: `ctl side${b?.hold ? ' bound-layer' : ''}`,
    'aria-pressed': String(state.selected === control),
    onclick: () => select(control),
  }, b?.label || (b?.hold ? 'Hold' : (b ? 'In use' : 'Empty')));
  wireDrag(el, control);
  return el;
}

function knob(control) {
  const b = binding(control);
  const el = h('button', {
    class: 'ctl knob',
    'aria-pressed': String(state.selected === control),
    'aria-label': `${controlName(control)}${b?.label ? `: ${b.label}` : ''}`,
    onclick: () => select(control),
  });
  wireDrag(el, control);
  return h('div', { class: 'knob-slot' }, el, h('span', { class: 'knob-label' }, b?.label || (b ? 'In use' : 'Empty')));
}

function drawDeck() {
  const deck = document.querySelector('.deck');
  if (!deck) return;
  const shown = new Map();
  for (const el of deck.querySelectorAll('.key[data-control]')) {
    const img = el.querySelector('img');
    if (img) shown.set(el.dataset.control, img);
  }
  // Only the D200X has knobs and side buttons, and USB can't tell, so the Settings model decides.
  deck.classList.toggle('folder', !!page().folder);
  const extras = hasDials() ? [sideButton('side1'), sideButton('side2'), ...KNOBS.map(knob)] : [];
  deck.replaceChildren(
    ...KEYS.map((c) => keyButton(c, shown.get(c))), keyButton('wide', shown.get('wide')), ...extras,
  );
}

function actionLists(action, visit) {
  if (!action || typeof action === 'string') return;
  visit(action);
  actionLists(action.one_shot, visit);
  for (const list of [action.sequence, action.toggle, action.random]) list?.forEach((a) => actionLists(a, visit));
}

function forEachAction(pages, visit) {
  for (const p of pages) {
    for (const b of Object.values(p.keys)) {
      for (const key of ['action', 'double', 'hold', 'left', 'right', 'press_left', 'press_right']) actionLists(b[key], (a) => visit(a, p));
    }
  }
}

const allPages = () => state.cfg.profile.flatMap((p) => p.page);

function usesOfPage(name) {
  const uses = [];
  forEachAction(profile().page, (a, p) => { if (a.page === name || a.show === name) uses.push(p.name); });
  return [...new Set(uses)];
}

function renamePage(name) {
  const p = page();
  if (!name || name === p.name) return draw();
  if (profile().page.some((o) => o.name === name)) {
    notify(`This profile already has a page named ${name}.`, true);
    return draw();
  }
  const old = p.name;
  forEachAction(profile().page, (a) => {
    if (a.page === old) a.page = name;
    if (a.show === old) a.show = name;
  });
  p.name = name;
  save();
  draw();
}

function deletePage() {
  if (!page().folder && profile().page.filter((p) => !p.folder).length === 1) return notify('A profile needs at least one page.', true);
  const uses = usesOfPage(page().name);
  if (uses.length) return notify(`Still used by: ${uses.join(', ')}. Remove those first.`, true);
  confirmDialog(`Delete ${page().name}?`, 'Everything set on this page goes with it.', 'Delete page', () => {
    profile().page.splice(state.page, 1);
    state.page = Math.max(0, state.page - 1);
    state.selected = null;
    save();
    draw();
  });
}

function duplicatePage() {
  const pages = profile().page;
  if (page().folder) return addFolderCopy();
  let name = ` copy`;
  while (pages.some((p) => p.name === name)) name += ' copy';
  pages.splice(state.page + 1, 0, { ...structuredClone(page()), name });
  state.page += 1;
  state.selected = null;
  save();
  draw();
}

function addFolderCopy() {
  const pages = profile().page;
  let name = ` copy`;
  while (pages.some((p) => p.name === name)) name += ' copy';
  pages.push({ ...structuredClone(page()), name });
  state.page = pages.length - 1;
  state.selected = null;
  save();
  draw();
}

function startRename() {
  const tab = document.querySelector('.tab[aria-current="true"]');
  if (!tab) return;
  let cancelled = false;
  const input = h('input', {
    class: 'tab-input', value: page().name, 'aria-label': 'Page name',
    onkeydown: (e) => {
      if (e.key === 'Enter') input.blur();
      if (e.key === 'Escape') { e.stopPropagation(); cancelled = true; input.blur(); }
    },
    onblur: () => (cancelled ? draw() : renamePage(input.value.trim())),
  });
  input.setAttribute('type', 'text');
  tab.replaceWith(input);
  input.select();
}

function pageMenu(at, index) {
  state.page = index;
  state.selected = null;
  draw();
  const p = page();
  openMenu(at, [
    { label: 'Rename', icon: 'pencil-simple-bold', run: startRename },
    { label: 'Duplicate', icon: 'copy-bold', run: duplicatePage },
    p.folder ? null : { label: 'Skip in Next and Previous page', icon: 'skip-forward-bold', checked: !!p.hidden, run: () => { p.hidden = !p.hidden; save(); draw(); } },
    '-',
    { label: p.folder ? 'Delete folder' : 'Delete page', icon: 'trash-bold', danger: true, run: deletePage },
  ]);
}

function byKind() {
  // Normal pages first: a profile opens on its first page, which can't be a folder.
  profile().page.sort((a, b) => !!a.folder - !!b.folder);
}

function freeName(base) {
  let n = profile().page.length + 1;
  while (profile().page.some((p) => p.name === `${base} ${n}`)) n++;
  return `${base} ${n}`;
}

function newFolder() {
  return { name: freeName('Folder'), folder: true, keys: {} };
}

function addFolder() {
  const folder = newFolder();
  profile().page.push(folder);
  state.page = profile().page.indexOf(folder);
  state.selected = null;
  save();
  draw();
}

/** A new page starts with the first page's knobs and side buttons. */
function addPage() {
  const pages = profile().page;
  const keys = {};
  for (const control of [...KNOBS, 'side1', 'side2']) {
    if (pages[0].keys[control]) keys[control] = structuredClone(pages[0].keys[control]);
  }
  const fresh = { name: freeName('Page'), keys };
  pages.push(fresh);
  byKind();
  state.page = pages.indexOf(fresh);
  state.selected = null;
  save();
  draw();
}

function pageTabs() {
  const tabs = profile().page.map((p, i) => {
    const tab = h('button', {
      class: p.hidden ? 'tab skipped' : 'tab',
      'aria-current': String(i === state.page),
      onclick: () => { state.page = i; state.selected = null; draw(); },
      ondblclick: startRename,
      oncontextmenu: (e) => { e.preventDefault(); e.stopPropagation(); pageMenu(menuPoint(e), i); },
    }, p.folder ? icon('folder-bold') : null, p.name);
    return tab;
  });
  return h('div', { class: 'tabs', role: 'group', 'aria-label': 'Pages' }, ...tabs, iconButton('plus-bold', 'Add page', addPage), iconButton('folder-plus-bold', 'Add folder', addFolder));
}

function defaultProfile() {
  const list = state.cfg.profile;
  const marked = list.findIndex((p) => p.default);
  if (marked >= 0) return marked;
  return Math.max(0, list.findIndex((p) => !p.exe && !p.title && !p.title_regex));
}

function setDefault(index) {
  state.cfg.profile.forEach((p, i) => { p.default = i === index; });
  save();
  drawProfiles();
}

function profileMenu(at, index) {
  const last = state.cfg.profile.length - 1;
  openMenu(at, [
    { label: 'Profile settings', icon: 'sliders-horizontal-bold', run: () => profileDialog(index) },
    { label: 'Set as default', icon: 'house-bold', checked: index === defaultProfile(), run: () => setDefault(index) },
    { label: 'Move up', icon: 'arrow-up-bold', disabled: index === 0, run: () => moveProfile(index, index - 1) },
    { label: 'Move down', icon: 'arrow-down-bold', disabled: index === last, run: () => moveProfile(index, index + 1) },
    '-',
    { label: 'Delete profile', icon: 'trash-bold', danger: true, run: () => deleteProfile(index) },
  ]);
}

function drawKeysView() {
  const more = iconButton('dots-three-bold', 'Profile options', (e) => profileMenu(menuPoint(e), state.profile));
  const head = h('div', { class: 'bench-head' }, h('div', { class: 'profile-bar' }, h('h1', {}, profile().name), more), pageTabs());
  const bench = h('div', {
    class: 'bench',
    onclick: (e) => { if (e.target === bench) select(null); },
  }, head, h('div', { class: 'deck' }));
  $('stage').replaceChildren(bench);
  drawDeck();
}

// ---------------------------------------------------------------- action list
function drawActions() {
  let index = 0;
  const entries = [];
  const groups = ACTION_GROUPS.map(([name, items]) => {
    const rows = items.map((item) => {
      const position = index++;
      if (item.knob && !hasDials()) return null;
      const row = h('button', {
        class: item.hold ? 'action hold' : 'action',
        draggable: true,
        onclick: () => applyPalette(item, state.selected),
      }, icon(item.icon), item.name);
      row.addEventListener('dragstart', (e) => e.dataTransfer.setData('text/plain', `palette:${position}`));
      return row;
    }).filter(Boolean);
    if (!rows.length) return null;
    const group = h('details', {
      class: 'group',
      open: !state.collapsed.has(name),
      ontoggle: () => {
        if (search.value) return; // searching opens every group; that isn't the user's choice
        if (group.open) state.collapsed.delete(name);
        else state.collapsed.add(name);
        remember('collapsed', JSON.stringify([...state.collapsed]));
      },
    }, h('summary', {}, name), ...rows);
    entries.push({ name, group, rows });
    return group;
  });
  const empty = h('p', { class: 'hint', hidden: true }, 'No action matches that search.');
  const search = h('input', {
    type: 'search', placeholder: 'Search actions', 'aria-label': 'Search actions',
    oninput: () => {
      const words = search.value.toLowerCase().split(/\s+/).filter(Boolean);
      let shown = 0;
      for (const entry of entries) {
        const inGroup = words.every((w) => entry.name.toLowerCase().includes(w));
        const visible = entry.rows.filter((row) => {
          const match = inGroup || words.every((w) => row.textContent.toLowerCase().includes(w));
          row.hidden = !match;
          return match;
        }).length;
        entry.group.hidden = visible === 0;
        entry.group.open = words.length ? true : !state.collapsed.has(entry.name);
        shown += visible;
      }
      empty.hidden = shown > 0;
    },
  });
  $('actions').replaceChildren(h('div', { class: 'actions-search' }, search), empty, ...groups.filter(Boolean));
}

// ---------------------------------------------------------------- dialogs
function openDialog(...content) {
  const dialog = $('dialog');
  dialog.classList.remove('narrow');
  dialog.replaceChildren(...content);
  if (!dialog.open) dialog.showModal();
  return dialog;
}

function iconPicker(onPick) {
  const names = [...state.icons.keys()];
  const search = h('input', { type: 'search', placeholder: 'Search icons in English', 'aria-label': 'Search icons' });
  const style = h('select', { 'aria-label': 'Icon style' }, h('option', { value: 'bold' }, 'Bold'), h('option', { value: 'fill' }, 'Filled'));
  const grid = h('div', { class: 'icon-grid' });
  const count = h('p', { class: 'hint' });
  const LIMIT = 240;
  const refresh = () => {
    const words = search.value.toLowerCase().split(/\s+/).filter(Boolean);
    const found = names.filter((n) => n.endsWith(`-${style.value}`) && words.every((w) => n.includes(w)));
    grid.replaceChildren(...found.slice(0, LIMIT).map((name) => h('button', {
      class: 'icon-choice',
      title: name.replace(/-(bold|fill)$/, ''),
      onclick: () => { $('dialog').close(); onPick(`ph:${name}`); },
    }, icon(name))));
    count.textContent = found.length > LIMIT ? `Showing ${LIMIT} of ${found.length}. Search to narrow it down.` : `${found.length} icons`;
  };
  search.addEventListener('input', refresh);
  style.addEventListener('change', refresh);
  const own = h('button', {
    class: 'plain',
    onclick: async () => {
      const path = await invoke('pick_file', { name: 'Icons', extensions: ['svg', 'png'] });
      if (path) { $('dialog').close(); onPick(path); }
    },
  }, 'Use my own SVG or PNG');
  openDialog(
    h('h2', {}, 'Choose an icon'),
    h('div', { class: 'row' }, search, style, own),
    grid, count,
    h('button', { class: 'plain done', onclick: () => $('dialog').close() }, 'Cancel'),
  );
  refresh();
  search.focus();
}

function keyboardPicker(current, onSave) {
  const tokens = current ? current.split('+').map((t) => t.trim().toLowerCase()).filter(Boolean) : [];
  const preferred = recall('keyboard') ?? (navigator.language === 'pt-BR' ? 'Brazilian (ABNT2)' : 'US (ANSI)');
  const layout = h('select', { 'aria-label': 'Keyboard layout' }, ...Object.keys(LAYOUTS).map((n) => h('option', { value: n, selected: n === preferred }, n)));
  const combo = h('p', { class: 'combo' });
  const board = h('div', { class: 'board' });

  const toggle = (token) => {
    const at = tokens.indexOf(token);
    if (at >= 0) tokens.splice(at, 1);
    else tokens.push(token);
    refresh();
  };
  const keyRow = (keys) => h('div', { class: 'kb-row' }, ...keys.map(([label, token, width]) => (label === null
    ? h('span', { class: 'kb-gap', style: `--w:${width}` })
    : h('button', { class: 'kb-key', style: `--w:${width}`, 'aria-pressed': String(tokens.includes(token)), onclick: () => toggle(token) }, label))));
  const hex = h('input', { type: 'text', maxLength: 2, placeholder: 'EA', 'aria-label': 'Key code in hex' });
  const addHex = () => {
    const code = parseInt(hex.value, 16);
    if (!code || code > 255) return notify('A key code is a hex number from 01 to FF.', true);
    toggle(`vk${code.toString(16).padStart(2, '0')}`);
    hex.value = '';
  };
  function refresh() {
    combo.textContent = tokens.length ? tokens.join(' + ') : 'Click keys in the order they should be pressed';
    remember('keyboard', layout.value);
    board.replaceChildren(
      h('div', { class: 'kb-block' }, ...FUNCTION_ROWS.map(keyRow), ...LAYOUTS[layout.value].map(keyRow)),
      h('div', { class: 'kb-side' },
        h('div', { class: 'kb-block small' }, ...NAV_BLOCK.map(keyRow)),
        h('div', { class: 'kb-block small' }, ...NUMPAD.map(keyRow))),
      h('div', { class: 'kb-media' }, keyRow(MEDIA_KEYS)),
    );
  }
  layout.addEventListener('change', refresh);
  openDialog(
    h('h2', {}, 'Choose keys'),
    h('div', { class: 'row' }, layout, h('span', { class: 'kb-hex' }, 'Other key code'), hex, h('button', { onclick: addHex }, 'Add')),
    combo, board,
    h('div', { class: 'row done' },
      h('button', { class: 'plain', onclick: () => { tokens.length = 0; refresh(); } }, 'Clear'),
      h('button', { class: 'plain', onclick: () => $('dialog').close() }, 'Cancel'),
      h('button', { class: 'plain primary', onclick: () => { $('dialog').close(); onSave(tokens.join('+')); } }, 'Use these keys')),
  );
  refresh();
}

// ---------------------------------------------------------------- inspector
const ACTION_TYPES = [
  ['none', 'Nothing'], ['hotkey', 'Hotkey'], ['text', 'Type text'], ['open', 'Open app, file or website'], ['close', 'Close app'],
  ['page', 'Open page'], ['back', 'Go back'], ['profile', 'Switch profile'], ['next_page', 'Next page'], ['prev_page', 'Previous page'],
  ['scroll_up', 'Scroll up'], ['scroll_down', 'Scroll down'], ['scroll_left', 'Scroll left'], ['scroll_right', 'Scroll right'],
  ['brightness', 'Change deck brightness'], ['app_volume', 'Change app volume'], ['app_volume_reset', 'Reset app volume'], ['play_sound', 'Play sound file'], ['stop_sound', 'Stop sound'],
  ['sequence', 'Sequence'], ['toggle', 'Toggle between actions'], ['random', 'Random action'],
];
const DELAY_TYPE = ['delay', 'Wait'];
const SHOW_TYPE = ['show', 'Show a page while held'];
const actionType = (a) => (!a ? 'none' : typeof a === 'string' ? a : Object.keys(a)[0]);

const ACTION_NAMES = Object.fromEntries([...ACTION_TYPES, DELAY_TYPE, SHOW_TYPE]);

/** The action a list entry produces, so filled slots can reuse its icon. */
function paletteResult(item) {
  const result = {};
  item.apply(result, context);
  return result;
}

function actionIcon(type) {
  const item = PALETTE.find((entry) => {
    const { action, hold } = paletteResult(entry);
    return actionType(hold?.show === undefined ? action : hold) === type && !entry.knob;
  });
  return item ? icon(item.icon) : null;
}

/** Turns a list entry into a slot's value, or explains why it doesn't fit there. */
function fillSlot(item, options, commit, decorate) {
  ensureFolder(item);
  const result = paletteResult(item);
  if (item.knob || item.screen) return notify(` cannot go in an action slot. Drop it on a control.`, true);
  if (item.wait && !options.delay) return notify('Wait only works inside a sequence.', true);
  if (result.hold !== undefined) {
    if (result.hold.show === undefined) return notify('Drag the action to run into this slot.', true);
    if (!options.show) return notify('Showing a page only works in the When held slot.', true);
    decorate?.(result);
    return commit(result.hold);
  }
  decorate?.(result);
  commit(result.action);
}

function pageOptions(selected, extra = []) {
  return [...extra, ...profile().page.map((p) => [p.name, p.name])]
    .map(([value, text]) => h('option', { value, selected: value === selected }, text));
}

function browseButton(extensions, commit) {
  return h('button', {
    type: 'button',
    onclick: async () => {
      const path = await invoke('pick_file', { name: 'Files', extensions });
      if (path) commit(path);
    },
  }, 'Browse');
}

function listEditor(key, action, commit, inSequence) {
  const list = action[key];
  const update = (next, quiet) => commit({ [key]: next }, quiet);
  const move = (from, to) => {
    const next = [...list];
    next.splice(to, 0, next.splice(from, 1)[0]);
    update(next);
  };
  const mover = (i, to, name, glyph) => h('button', {
    type: 'button', class: 'slot-tool', title: name, 'aria-label': `Move step ${i + 1} ${name.toLowerCase()}`,
    disabled: to < 0 || to >= list.length,
    onclick: () => move(i, to),
  }, icon(glyph));
  const steps = list.map((step, i) => actionEditor(`Step ${i + 1}`, step, (value, quiet) => {
    const next = [...list];
    if (value === undefined) next.splice(i, 1);
    else next[i] = value;
    update(next, quiet);
  }, { delay: inSequence, tools: [mover(i, i - 1, 'Up', 'caret-up-bold'), mover(i, i + 1, 'Down', 'caret-down-bold')] }));
  const add = actionEditor('', undefined, (value) => update([...list, value]), { delay: inSequence, prompt: 'Drag an action here to add a step' });
  return h('div', { class: 'steps' }, ...steps, add);
}

/** A dotted slot that takes an action dragged from the list, or the next one clicked there. */
function actionEditor(label, action, commit, options = {}) {
  // One shot wraps the real action, so the editor below only ever sees the inner one.
  const plainCommit = commit;
  const shot = action?.one_shot !== undefined;
  if (shot) {
    action = action.one_shot;
    commit = (value, quiet) => plainCommit(value === undefined ? undefined : { one_shot: value }, quiet);
  }
  const type = actionType(action);
  const parts = [];
  const take = (item) => {
    state.armed = null;
    fillSlot(item, options, commit, options.decorate);
  };
  const slot = h('div', { class: action ? 'slot filled' : 'slot' });
  if (action) {
    slot.append(h('div', { class: 'slot-head' }, actionIcon(type), h('span', { class: 'slot-name' }, ACTION_NAMES[type] ?? type),
      ...(options.tools ?? []),
      h('button', { type: 'button', class: 'slot-tool', title: 'Remove', 'aria-label': `Remove ${ACTION_NAMES[type] ?? type}`, onclick: () => commit(undefined) }, icon('x-bold'))));
  } else {
    slot.append(h('button', {
      type: 'button', class: 'slot-empty',
      onclick: () => {
        const armed = state.armed === take;
        document.querySelectorAll('.slot.armed').forEach((el) => el.classList.remove('armed'));
        state.armed = armed ? null : take;
        slot.classList.toggle('armed', !armed);
        if (!armed) notify('Now click an action in the list.');
      },
    }, options.prompt ?? 'Drag an action here'));
  }
  slot.addEventListener('dragover', (e) => { e.preventDefault(); e.stopPropagation(); slot.classList.add('over'); });
  slot.addEventListener('dragleave', (e) => { e.stopPropagation(); slot.classList.remove('over'); });
  slot.addEventListener('drop', (e) => {
    e.preventDefault();
    e.stopPropagation();
    slot.classList.remove('over');
    const [kind, value] = e.dataTransfer.getData('text/plain').split(':');
    if (kind === 'palette') take(PALETTE[Number(value)]);
  });
  const textField = (key, placeholder) => {
    const input = h('input', { type: 'text', value: action[key], placeholder, oninput: () => commit({ [key]: input.value }, true) });
    return input;
  };
  if (type === 'hotkey') {
    const keys = action.hotkey ? action.hotkey.split('+').map((key) => h('kbd', {}, key.trim())) : ['Choose keys'];
    parts.push(h('button', {
      type: 'button', class: 'keys', title: 'Choose keys',
      onclick: () => keyboardPicker(action.hotkey, (value) => commit({ hotkey: value })),
    }, ...keys));
  } else if (type === 'text') {
    parts.push(textField('text', 'Text to type'));
  } else if (type === 'open') {
    parts.push(h('div', { class: 'row' }, textField('open', 'Path or https://…'), browseButton([], (path) => commit({ open: path }))));
  } else if (type === 'close') {
    parts.push(textField('close', 'Program, like notepad.exe'));
    parts.push(h('p', { class: 'hint' }, 'Leave it empty to close the focused window.'));
  } else if (type === 'page') {
    const select = h('select', { onchange: () => commit({ page: select.value }) }, ...pageOptions(action.page));
    parts.push(select);
    parts.push(h('p', { class: 'hint' }, 'Put a Go back action on that page to return here.'));
  } else if (type === 'show') {
    const select = h('select', { 'aria-label': 'Page to show', onchange: () => commit({ show: select.value }) },
      ...pageOptions(action.show, [[APP_LAYER, 'Page of the focused app']]));
    parts.push(select);
  } else if (type === 'profile') {
    const select = h('select', { onchange: () => commit({ profile: select.value }) },
      ...[[APP_LAYER, "Focused app's profile"], [DEFAULT_PROFILE, 'Default profile'], ...state.cfg.profile.map((p) => [p.name, p.name])]
        .map(([value, text]) => h('option', { value, selected: value === action.profile }, text)));
    parts.push(select);
  } else if (type.startsWith('scroll_')) {
    const input = h('input', { type: 'number', value: action[type], min: 1, max: 100, onchange: () => commit({ [type]: Math.max(1, Number(input.value) || 1) }) });
    parts.push(field('Notches', input));
  } else if (type === 'app_volume') {
    const input = h('input', { type: 'number', value: action.app_volume, min: -100, max: 100, onchange: () => commit({ app_volume: Number(input.value) || 0 }) });
    parts.push(field('Change by (%)', input, 'Applies to the window that has focus.'));
  } else if (type === 'app_volume_reset') {
    parts.push(h('p', { class: 'hint' }, 'Sets the focused app to 100%, so it follows the system volume again.'));
  } else if (type === 'brightness') {
    const input = h('input', { type: 'number', value: action.brightness, min: -100, max: 100, onchange: () => commit({ brightness: Number(input.value) || 0 }) });
    parts.push(field('Change by', input));
  } else if (type === 'play_sound') {
    parts.push(h('div', { class: 'row' }, textField('play_sound', 'WAV or MP3 file'), browseButton(['wav', 'mp3'], (path) => commit({ play_sound: path }))));
  } else if (type === 'delay') {
    const input = h('input', { type: 'number', value: action.delay, min: 0, step: 50, onchange: () => commit({ delay: Number(input.value) || 0 }) });
    parts.push(field('Milliseconds', input));
  } else if (type === 'sequence') {
    parts.push(listEditor('sequence', action, commit, true));
  } else if (type === 'toggle' || type === 'random') {
    parts.push(h('p', { class: 'hint' }, type === 'toggle' ? 'Each press runs the next step.' : 'Each press runs one step at random.'));
    parts.push(listEditor(type, action, commit, false));
  }
  if (type === 'page' || type === 'profile') {
    const box = h('input', { type: 'checkbox', checked: shot, onchange: () => plainCommit(box.checked ? { one_shot: action } : action) });
    parts.push(h('label', { class: 'check one-shot', title: 'Goes back after the next button press' }, box, 'One shot'));
    if (shot) parts.push(h('p', { class: 'hint' }, 'Returns here after the next button press.'));
  }
  slot.append(...parts);
  return h('div', { class: 'slot-field' }, label ? h('span', { class: 'slot-label' }, label) : null, slot);
}

/** `quiet` keeps the input focused while typing; the deck still repaints. */
function setter(control, key) {
  return (value, quiet) => {
    edit(control, (b) => { b[key] = value; });
    if (!quiet) drawInspector();
  };
}

function displaySection(control, b) {
  const set = setter(control, 'display');
  if (control === 'wide') {
    const current = wideDisplay(b);
    const select = h('select', { onchange: () => set(select.value) },
      ...WIDE_DISPLAYS.map(([value, text]) => h('option', { value, selected: value === current }, text)));
    return field('Show', select);
  }
  const box = h('input', { type: 'checkbox', checked: b?.display === 'page_indicator', onchange: () => set(box.checked ? 'page_indicator' : undefined) });
  return h('label', { class: 'check' }, box, 'Show the page number');
}

function iconTile(control, b) {
  const setIcon = setter(control, 'icon');
  const file = () => invoke('pick_file', { name: 'Icons', extensions: ['svg', 'png'] }).then((path) => { if (path) setIcon(path); });
  const library = b?.icon?.startsWith('ph:');
  const face = library ? icon(b.icon.slice(3)) : b?.icon ? 'File' : icon('plus-bold');
  const tile = h('button', {
    type: 'button',
    class: `icon-tile${b?.icon ? (library ? '' : ' file') : ' empty'}`,
    title: b?.icon && !library ? b.icon : 'Choose icon',
    'aria-label': 'Choose icon',
    onclick: () => iconPicker((value) => setIcon(value)),
    oncontextmenu: (e) => {
      e.preventDefault();
      e.stopPropagation();
      openMenu(menuPoint(e), [
        { label: 'Choose icon', icon: 'image-bold', run: () => iconPicker((value) => setIcon(value)) },
        { label: 'Use my own file', icon: 'folder-open-bold', run: file },
        '-',
        { label: 'Remove icon', icon: 'x-bold', danger: true, disabled: !b?.icon, run: () => setIcon(undefined) },
      ]);
    },
  }, face);
  return tile;
}

function appearance(control, b) {
  const set = setter(control, 'label');
  const label = h('input', { type: 'text', value: b?.label ?? '', oninput: () => set(label.value, true) });
  if (!hasScreen(control)) return [field('Name', label)];

  const parts = [displaySection(control, b)];
  if (control === 'wide' && !['image', 'page_indicator'].includes(wideDisplay(b))) {
    parts.push(h('p', { class: 'hint' }, 'The deck draws this face itself, so the label, icon and colours do not apply.'));
    return parts;
  }
  const swatch = (key, title, fallback) => {
    // Dragging only previews here; the deck gets the colour when the picker settles.
    const input = h('input', {
      type: 'color', value: b?.[key] ?? fallback, 'aria-label': title,
      oninput: () => edit(control, (x) => { x[key] = input.value; }, false),
      onchange: save,
    });
    return h('label', {}, input, title);
  };
  parts.push(h('div', { class: 'looks' }, iconTile(control, b),
    h('div', { class: 'swatches' }, swatch('color', 'Icon', state.cfg.color), swatch('background', 'Background', state.cfg.background))));
  parts.push(field('Label', label));
  return parts;
}

function holdSection(control, b) {
  const parts = [h('h3', {}, 'When held'), actionEditor('', b?.hold, setter(control, 'hold'), { show: true, decorate: decorator(control) })];
  if (!b?.hold) return parts;
  const shows = b.hold.show !== undefined;
  const instant = shows && !b.action && !b.double;
  if (instant) {
    parts.push(h('p', { class: 'hint' }, 'The page appears as soon as this is pressed, and goes away on release.'));
    return parts;
  }
  const setHold = setter(control, 'hold_ms');
  const fallback = shows ? state.cfg.hold_ms : state.cfg.hold_action_ms;
  const time = h('input', {
    type: 'number', min: 50, max: 5000, step: 50, value: b.hold_ms ?? fallback,
    onchange: () => setHold(Number(time.value) || null, true),
  });
  parts.push(field('Hold for (ms)', time, shows
    ? 'A quick tap runs the click action. Holding longer, or pressing another key meanwhile, shows the page.'
    : 'A border draws around the key, then the action runs. Letting go early cancels it.'));
  return parts;
}

/** Dropping an action also fills in its default label and icon if the control has none. */
function decorator(control) {
  return (result) => edit(control, (b) => {
    b.label ||= result.label;
    b.icon ||= result.icon;
  });
}

function drawInspector() {
  state.armed = null;
  const root = $('inspector');
  const control = state.selected;
  if (state.view !== 'keys') return root.replaceChildren();
  if (control === null) {
    return root.replaceChildren(h('p', { class: 'panel-empty' }, 'Select a key, side button or knob to set it up.'));
  }

  const b = binding(control);
  const parts = [
    h('div', { class: 'inspector-head' }, h('h2', {}, controlName(control)),
      iconButton('dots-three-bold', 'More options', (e) => controlMenu(menuPoint(e), control)),
      iconButton('x-bold', 'Close', () => select(null))),
    ...appearance(control, b),
  ];
  if (isKnob(control)) {
    parts.push(h('h3', {}, 'Actions'));
    for (const [key, label] of [['action', 'Click'], ['double', 'Double click'], ['left', 'Turn left'], ['right', 'Turn right'], ['press_left', 'Turn left while pressed'], ['press_right', 'Turn right while pressed']]) {
      parts.push(actionEditor(label, b?.[key], setter(control, key), { decorate: key === 'action' ? decorator(control) : undefined }));
    }
  } else {
    parts.push(h('h3', {}, b?.hold ? 'When tapped' : 'When pressed'));
    parts.push(actionEditor('', b?.action, setter(control, 'action'), { decorate: decorator(control) }));
    parts.push(h('h3', {}, 'When double clicked'));
    parts.push(actionEditor('', b?.double, setter(control, 'double')));
    if (b?.double) parts.push(h('p', { class: 'hint' }, `The single click runs ${state.cfg.double_ms} ms later, to rule out a second click.`));
    parts.push(...holdSection(control, b));
  }
  root.replaceChildren(...parts);
}

// ---------------------------------------------------------------- other views
function renameProfile(target, name) {
  if (!name || name === target.name) return;
  if (state.cfg.profile.some((p) => p.name === name)) return notify(`A profile named ${name} already exists.`, true);
  forEachAction(allPages(), (a) => { if (a.profile === target.name) a.profile = name; });
  target.name = name;
  save();
  draw();
}

function deleteProfile(index) {
  const target = state.cfg.profile[index];
  if (state.cfg.profile.length === 1) return notify('The deck needs at least one profile.', true);
  const uses = [];
  forEachAction(allPages(), (a, p) => { if (a.profile === target.name) uses.push(p.name); });
  if (uses.length) return notify(`Still used by: ${[...new Set(uses)].join(', ')}. Remove those first.`, true);
  confirmDialog(`Delete ${target.name}?`, 'All of its pages go with it.', 'Delete profile', () => {
    state.cfg.profile.splice(index, 1);
    state.profile = 0;
    state.page = 0;
    state.selected = null;
    save();
    draw();
  });
}

function profileDialog(index) {
  const target = state.cfg.profile[index];
  const name = h('input', { type: 'text', value: target.name, onchange: () => renameProfile(target, name.value.trim()) });
  const text = (key, label, hint) => {
    const input = h('input', {
      type: 'text', value: target[key] ?? '',
      oninput: () => {
        if (input.value) target[key] = input.value;
        else delete target[key];
        save();
        drawProfiles();
      },
    });
    return field(label, input, hint);
  };
  const flag = (key, label) => {
    const box = h('input', { type: 'checkbox', checked: target[key] ?? true, onchange: () => { target[key] = box.checked; save(); } });
    return h('label', { class: 'check' }, box, label);
  };
  openDialog(
    h('h2', {}, 'Profile settings'),
    h('div', { class: 'profile-form' },
      field('Name', name),
      h('h3', {}, 'Use this profile for'),
      h('p', { class: 'hint' }, 'Every field you fill in has to match, and the first matching profile from the top wins. Leave them all empty for a profile that only opens through an action or as the default.'),
      text('exe', 'Program', 'For example zed.exe, or a full path.'),
      text('title', 'Window title contains'),
      text('title_regex', 'Window title matches regex'),
      flag('on_focus', 'Switch to it automatically while the app has focus'),
      flag('on_hold', 'Show its first page while holding a focused-app key')),
    h('div', { class: 'row done' },
      h('button', { class: 'plain primary', onclick: () => $('dialog').close() }, 'Done')),
  );
}

const DATE_STYLES = [['', 'System format'], ['dd/MM/yyyy', '31/12/2026'], ['MM/dd/yyyy', '12/31/2026'], ['yyyy-MM-dd', '2026-12-31'], ['dd/MM', '31/12'], ['d MMM yyyy', '31 Dec 2026'], ['d MMM', '31 Dec']];

function dateField() {
  const cfg = state.cfg;
  const known = DATE_STYLES.some(([value]) => value === cfg.date_format);
  const custom = h('input', {
    type: 'text', value: cfg.date_format, placeholder: 'dd/MM/yyyy', hidden: known,
    oninput: () => { cfg.date_format = custom.value; save(); },
  });
  const style = h('select', {
    onchange: () => {
      const own = style.value === 'custom';
      custom.hidden = !own;
      if (!own) { cfg.date_format = style.value; save(); } else custom.focus();
    },
  }, ...DATE_STYLES.map(([value, text]) => h('option', { value, selected: value === cfg.date_format }, text)),
  h('option', { value: 'custom', selected: !known }, 'Custom'));
  return field('Date', h('div', { class: 'date-style' }, style, custom), 'Custom uses d, M and y, as in dd/MM/yy. Names follow the Windows language.');
}

function forgetButton() {
  const deck = state.devices.find((d) => d.id === state.device);
  if (!deck || deck.connected || !deck.id) return null;
  return h('button', {
    class: 'plain forget',
    onclick: () => confirmDialog(`Forget ${deck.label}?`, 'Its profiles and pages are deleted. If this deck is plugged in again, it starts from the shared config.', 'Forget deck', async () => {
      clearTimeout(saveTimer); // a pending save would bring the file straight back
      saveTimer = null;
      try {
        await invoke('forget_device', { device: deck.id });
      } catch (e) {
        return notify(`Not forgotten: ${e}`, true);
      }
      await loadDevices();
      state.view = 'keys';
      await switchDevice(firstDevice());
    }),
  }, 'Forget this deck');
}

function drawSettingsView() {
  const cfg = state.cfg;
  const number = (key, label, min, max, hint) => {
    const input = h('input', { type: 'number', min, max, value: cfg[key], onchange: () => { cfg[key] = Number(input.value); save(); } });
    return field(label, input, hint);
  };
  const toggle = (key, label) => {
    const input = h('input', { type: 'checkbox', checked: cfg[key], onchange: () => { cfg[key] = input.checked; save(); } });
    return h('label', { class: 'check' }, input, label);
  };
  const color = (key, label) => {
    const input = h('input', { type: 'color', value: cfg[key], onchange: () => { cfg[key] = input.value; save(); } });
    return h('label', { class: 'check' }, input, label);
  };
  const wide = h('select', { onchange: () => { cfg.wide = wide.value; save(); } },
    ...WIDE_DISPLAYS.filter(([id]) => id !== 'page_indicator').map(([value, text]) => h('option', { value, selected: value === cfg.wide }, text === 'Label and icon' ? 'Nothing' : text)));
  const weekday = h('select', { onchange: () => { cfg.weekday_format = weekday.value; save(); } },
    ...[['ddd', 'Short name'], ['dddd', 'Full name']].map(([value, text]) => h('option', { value, selected: value === cfg.weekday_format }, text)));
  const name = h('input', {
    type: 'text', value: cfg.name ?? '', placeholder: 'Named after its model',
    onchange: async () => { cfg.name = name.value.trim(); save(); await flushSave(); loadDevices(); },
  });
  const model = h('select', {
    onchange: async () => { cfg.model = model.value; save(); await flushSave(); loadDevices(); drawActions(); },
  }, ...[['d200x', 'D200X, with knobs and side buttons'], ['d200h', 'D200H, keys only'], ['d200', 'D200, keys only']]
    .map(([value, text]) => h('option', { value, selected: value === cfg.model }, text)));
  $('stage').replaceChildren(h('div', { class: 'sheet' },
    h('h1', {}, 'Settings'),
    h('p', {}, 'These apply to this deck. Start at login is in the tray menu.'),
    field('Deck name', name),
    field('Model', model, 'The deck cannot report its model, so this decides which controls are shown.'),
    number('brightness', 'Brightness', 0, 100),
    number('locked_brightness', 'Brightness while the PC is locked', 0, 100),
    number('hold_ms', 'Hold to show a page (ms)', 50, 2000, 'For keys that also have a click action.'),
    number('hold_action_ms', 'Hold to run an action (ms)', 150, 5000, 'Keys can set their own time.'),
    number('double_ms', 'Double click gap (ms)', 100, 1000, 'Single clicks on keys with a double click action wait this long.'),
    field('Wide screen on pages that do not set it', wide),
    toggle('twelve_hour', 'Use a 12-hour clock'),
    dateField(),
    field('Weekday', weekday),
    color('color', 'Default icon colour'),
    color('background', 'Default key background'),
    forgetButton()));
}

// ---------------------------------------------------------------- shell
function moveProfile(from, to) {
  const list = state.cfg.profile;
  if (to < 0 || to >= list.length || from === to) return;
  const current = list[state.profile];
  list.splice(to, 0, list.splice(from, 1)[0]);
  state.profile = list.indexOf(current);
  save();
  drawProfiles();
  document.querySelectorAll('.profile')[to]?.focus();
}

function profileHint(p, index) {
  const rule = p.exe || p.title || p.title_regex;
  if (index === defaultProfile()) return rule ? `Default, ${rule}` : 'Default';
  return rule || 'Opened by an action';
}

function addProfile() {
  let n = state.cfg.profile.length + 1;
  while (state.cfg.profile.some((p) => p.name === `Profile ${n}`)) n++;
  state.cfg.profile.push({ name: `Profile ${n}`, on_focus: true, on_hold: true, page: [{ name: 'Page 1', keys: {} }] });
  state.profile = state.cfg.profile.length - 1;
  state.page = 0;
  state.view = 'keys';
  state.selected = null;
  save();
  draw();
  profileDialog(state.profile);
}

function drawProfiles() {
  const rows = state.cfg.profile.map((p, i) => {
    const row = h('button', {
      class: 'profile',
      draggable: true,

      'aria-current': String(state.view === 'keys' && i === state.profile),
      onclick: () => { state.profile = i; state.page = 0; state.view = 'keys'; state.selected = null; draw(); },
      oncontextmenu: (e) => { e.preventDefault(); e.stopPropagation(); profileMenu(menuPoint(e), i); },
      onkeydown: (e) => {
        const step = e.altKey ? { ArrowUp: -1, ArrowDown: 1 }[e.key] : 0;
        if (step) { e.preventDefault(); moveProfile(i, i + step); }
      },
    }, h('span', { class: 'profile-name' }, p.name), h('small', {}, profileHint(p, i)));
    row.addEventListener('dragstart', (e) => e.dataTransfer.setData('text/plain', `profile:${i}`));
    row.addEventListener('dragover', (e) => { e.preventDefault(); row.classList.add('over'); });
    row.addEventListener('dragleave', () => row.classList.remove('over'));
    row.addEventListener('drop', (e) => {
      e.preventDefault();
      row.classList.remove('over');
      const [kind, from] = e.dataTransfer.getData('text/plain').split(':');
      if (kind === 'profile') moveProfile(Number(from), i);
    });
    return row;
  });
  $('profiles').replaceChildren(h('div', { class: 'section-head' }, h('h2', {}, 'Profiles'), iconButton('plus-bold', 'Add profile', addProfile)), ...rows);
}

function drawViews() {
  for (const b of document.querySelectorAll('.view')) {
    b.setAttribute('aria-current', String(b.dataset.view === state.view));
  }
}

function draw() {
  drawViews();
  drawProfiles();
  ({ keys: drawKeysView, settings: drawSettingsView })[state.view]();
  document.body.classList.toggle('no-panels', state.view !== 'keys');
  drawInspector();
}

function drawStatus(status) {
  state.status = status;
  const connection = $('connection');
  if (!status?.connected) {
    $('live-deck').replaceChildren();
    connection.className = 'connection';
    connection.textContent = 'Deck not connected';
    $('showing').className = 'showing';
    $('showing').textContent = 'Plug it in and close Ulanzi Studio.';
  } else {
    connection.className = 'connection on';
    connection.textContent = status.locked ? 'Deck locked' : 'Deck connected';
    const held = status.layers.length > 0;
    $('showing').className = held ? 'showing held' : 'showing';
    $('showing').textContent = held ? `Holding ${status.layers.at(-1)}` : `Showing ${status.profile}, ${status.page}`;
  }
  if (status) drawMiniDeck($('live-deck'), status);
}

async function start() {
  for (const b of document.querySelectorAll('.view')) {
    b.addEventListener('click', () => { state.view = b.dataset.view; draw(); });
    b.prepend(h('span', { 'data-glyph': 'gear-six-bold' }));
  }
  document.addEventListener('contextmenu', (e) => {
    if (!e.target.closest('input, textarea')) e.preventDefault();
    closeMenu();
  });
  document.addEventListener('pointerdown', (e) => { if (!e.target.closest('.menu')) closeMenu(); });
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    if (document.getElementById('menu')) closeMenu(true);
    else if (!$('dialog').open && state.selected !== null) select(null);
  });
  state.icons = new Map(await invoke('icon_library'));
  for (const slot of document.querySelectorAll('[data-glyph]')) slot.replaceWith(icon(slot.dataset.glyph));
  state.collapsed = new Set(JSON.parse(recall('collapsed') ?? '[]'));
  const { listen } = window.__TAURI__.event;
  $('device').addEventListener('change', () => switchDevice($('device').value));
  await loadDevices();
  await switchDevice(firstDevice());
  listen('deck-status', ({ payload }) => {
    const known = state.devices.find((d) => d.id === payload.device);
    if (known && known.connected !== payload.connected) {
      known.connected = payload.connected;
      drawDevices();
    }
    if (payload.device === state.device) drawStatus(payload);
  });
  listen('devices-changed', async () => {
    await loadDevices();
    // The starting config only stands in until a real deck shows up.
    if (state.device === '') await switchDevice(firstDevice());
  });
}

function firstDevice() {
  return (state.devices.find((d) => d.connected) ?? state.devices[0]).id;
}

function drawDevices() {
  const select = $('device');
  select.hidden = state.devices.length < 2;
  select.replaceChildren(...state.devices.map((d) => h('option', { value: d.id, selected: d.id === state.device },
    d.connected ? d.label : `${d.label} (not connected)`)));
}

async function loadDevices() {
  state.devices = await invoke('list_devices');
  drawDevices();
}

async function switchDevice(id) {
  await flushSave(); // pending edits belong to the deck being left
  state.device = id;
  drawDevices();
  try {
    state.cfg = await invoke('get_config', { device: id });
  } catch (e) {
    $('stage').replaceChildren(h('div', { class: 'sheet' }, h('h1', {}, 'The config file has a problem'), h('p', {}, String(e)), h('p', {}, 'Fix the file, then reopen OpenUlanzi.')));
    return;
  }
  state.profile = 0;
  state.page = 0;
  state.selected = null;
  drawActions();
  draw();
  drawStatus(await invoke('get_status', { device: id }));
}

start();
